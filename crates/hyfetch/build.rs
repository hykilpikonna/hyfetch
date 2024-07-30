use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

use indexmap::IndexMap;
use regex::Regex;
use unicode_normalization::UnicodeNormalization as _;

#[derive(Debug)]
struct AsciiDistro {
    pattern: String,
    art: String,
}

impl AsciiDistro {
    fn friendly_name(&self) -> String {
        self.pattern
            .split('|')
            .next()
            .expect("invalid distro pattern")
            .trim_matches(|c: char| c.is_ascii_punctuation() || c == ' ')
            .replace(['"', '*'], "")
    }
}

fn main() {
    let neofetch_path = Path::new(env!("CARGO_WORKSPACE_DIR")).join("neofetch");

    println!(
        "cargo:rerun-if-changed={neofetch_path}",
        neofetch_path = neofetch_path.display()
    );

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    export_distros(neofetch_path, out_path);
}

fn export_distros<P>(neofetch_path: P, out_path: &Path)
where
    P: AsRef<Path>,
{
    let distros = parse_ascii_distros(neofetch_path);
    let mut variants = IndexMap::with_capacity(distros.len());

    for distro in &distros {
        let variant = distro
            .friendly_name()
            .replace(|c: char| c.is_ascii_punctuation() || c == ' ', "_")
            .nfc()
            .collect::<String>();
        if variants.contains_key(&variant) {
            let variant_fallback = format!("{variant}_fallback");
            if variants.contains_key(&variant_fallback) {
                todo!("too many name clashes in ascii distro patterns: {variant}");
            }
            variants.insert(variant_fallback, distro);
            continue;
        }
        variants.insert(variant, distro);
    }

    let mut buf = r###"
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Distro {
"###
    .to_owned();

    for (variant, AsciiDistro { pattern, .. }) in &variants {
        write!(
            buf,
            r###"
    // {pattern})
    {variant},
"###,
        )
        .unwrap();
    }

    buf.push_str(
        r###"
}

impl Distro {
    pub fn detect<S>(name: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        let name = name.as_ref().to_lowercase();
"###,
    );

    for (variant, AsciiDistro { pattern, .. }) in &variants {
        let patterns = pattern.split('|').map(|s| s.trim());
        let mut conds = Vec::new();

        for m in patterns {
            let stripped = m.trim_matches(['*', '\'', '"']).to_lowercase();

            if stripped.contains(['*', '"']) {
                if let Some((prefix, suffix)) = stripped.split_once(r#""*""#) {
                    conds.push(format!(
                        r#"name.starts_with("{prefix}") && name.ends_with("{suffix}")"#
                    ));
                    continue;
                }
                todo!("cannot properly parse: {m}");
            }

            // Exact matches
            if m.trim_matches('*') == m {
                conds.push(format!(r#"name == "{stripped}""#));
                continue;
            }

            // Both sides are *
            if m.starts_with('*') && m.ends_with('*') {
                conds.push(format!(
                    r#"name.starts_with("{stripped}") || name.ends_with("{stripped}")"#
                ));
                continue;
            }

            // Ends with *
            if m.ends_with('*') {
                conds.push(format!(r#"name.starts_with("{stripped}")"#));
                continue;
            }

            // Starts with *
            if m.starts_with('*') {
                conds.push(format!(r#"name.ends_with("{stripped}")"#));
                continue;
            }
        }

        let condition = conds.join(" || ");

        write!(
            buf,
            r###"
        if {condition} {{
            return Some(Self::{variant});
        }}
"###
        )
        .unwrap();
    }

    buf.push_str(
        r###"
        None
    }

    pub fn ascii_art(&self) -> &str {
        let art = match self {
"###,
    );

    let quotes = "#".repeat(80);
    for (variant, AsciiDistro { art, .. }) in &variants {
        write!(
            buf,
            r###"
            Self::{variant} => r{quotes}"
{art}
"{quotes},
"###,
        )
        .unwrap();
    }

    buf.push_str(
        r###"
        };
        &art[1..art.len().checked_sub(1).unwrap()]
    }
}
"###,
    );

    fs::write(out_path.join("distros.rs"), buf).expect("couldn't write distros.rs");
}

/// Parses ascii distros from neofetch script.
fn parse_ascii_distros<P>(neofetch_path: P) -> Vec<AsciiDistro>
where
    P: AsRef<Path>,
{
    let neofetch_path = neofetch_path.as_ref();

    let nf = {
        let nf = fs::read_to_string(neofetch_path).expect("couldn't read neofetch script");

        // Get the content of "get_distro_ascii" function
        let (_, nf) = nf
            .split_once("get_distro_ascii() {\n")
            .expect("couldn't find get_distro_ascii function");
        let (nf, _) = nf
            .split_once("\n}\n")
            .expect("couldn't find end of get_distro_ascii function");

        let mut nf = nf.replace('\t', &" ".repeat(4));

        // Remove trailing spaces
        while nf.contains(" \n") {
            nf = nf.replace(" \n", "\n");
        }
        nf
    };

    let case_re = Regex::new(r"case .*? in\n").expect("couldn't compile case regex");
    let eof_re = Regex::new(r"EOF[ \n]*?;;").expect("couldn't compile eof regex");

    // Split by blocks
    let mut blocks = Vec::new();
    for b in case_re.split(&nf) {
        blocks.extend(eof_re.split(b).map(|sub| sub.trim()));
    }

    // Parse blocks
    fn parse_block(block: &str) -> Option<AsciiDistro> {
        let (block, art) = block.split_once("'EOF'\n")?;

        // Join \
        //
        // > A <backslash> that is not quoted shall preserve the literal value of the
        // > following character, with the exception of a <newline>. If a <newline>
        // > follows the <backslash>, the shell shall interpret this as line
        // > continuation. The <backslash> and <newline> shall be removed before
        // > splitting the input into tokens. Since the escaped <newline> is removed
        // > entirely from the input and is not replaced by any white space, it cannot
        // > serve as a token separator.
        // See https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_02_01
        let block = block.replace("\\\n", "");

        // Get case pattern
        let pattern = block
            .split('\n')
            .next()
            .and_then(|pattern| pattern.trim().strip_suffix(')'))?;

        // Unescape backslashes here because backslashes are escaped in neofetch
        // for printf
        let art = art.replace(r"\\", r"\");

        Some(AsciiDistro {
            pattern: pattern.to_owned(),
            art,
        })
    }
    blocks
        .iter()
        .filter_map(|block| parse_block(block))
        .collect()
}
