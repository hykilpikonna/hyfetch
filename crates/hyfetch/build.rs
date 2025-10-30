use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use fs_extra::dir::CopyOptions;
use heck::ToUpperCamelCase;
use indexmap::IndexMap;
use regex::Regex;
use serde::Deserialize;
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

fn anything_that_exist(paths: &[&Path]) -> Option<PathBuf> {
    paths.iter().copied().find(|p| p.exists()).map(Path::to_path_buf)
}

fn main() -> Result<()> {
    // Path hack to make file paths work in both workspace and manifest directory
    let dir = PathBuf::from(env::var_os("CARGO_WORKSPACE_DIR").unwrap_or_else(|| env::var_os("CARGO_MANIFEST_DIR").unwrap()));
    let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    for file in &["neofetch", "hyfetch/data"] {
        let src = anything_that_exist(&[
            &dir.join(file),
            &dir.join("../../").join(file),
        ]).context("couldn't find neofetch")?;
        let dst = o.join(file);
        println!("cargo:rerun-if-changed={}", src.display());

        // Copy either file or directory
        if src.is_dir() {
            let opt = CopyOptions { overwrite: true, copy_inside: true, ..CopyOptions::default() };
            println!("copying {} to {}", src.display(), dst.display());
            fs_extra::dir::copy(&src, &dst, &opt)?;
        }
        else { fs::copy(&src, &dst)?; }
    }

    preset_codegen(&o.join("hyfetch/data/presets.json"), &o.join("presets.rs"))?;
    export_distros(&o.join("neofetch"), &o)?;
    Ok(())
}

fn export_distros(neofetch_path: &Path, out_path: &Path) -> Result<()>
{
    let distros = parse_ascii_distros(neofetch_path)?;
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
"###.to_string();

    for (variant, AsciiDistro { pattern, .. }) in &variants {
        write!(buf, r###"
    // {pattern})
    {variant},
"###)?;
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

        write!(buf, r###"
        if {condition} {{
            return Some(Self::{variant});
        }}
"###)?;
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
        write!(buf, r###"
            Self::{variant} => r{quotes}"
{art}
"{quotes},
"###)?;
    }

    buf.push_str(
        r###"
        };
        &art[1..art.len().checked_sub(1).unwrap()]
    }
}
"###,
    );

    fs::write(out_path.join("distros.rs"), buf)?;
    Ok(())
}

/// Parses ascii distros from neofetch script.
fn parse_ascii_distros(neofetch_path: &Path) -> Result<Vec<AsciiDistro>>
{
    let nf = {
        let nf = fs::read_to_string(neofetch_path)?;

        // Get the content of "get_distro_ascii" function
        let (_, nf) = nf
            .split_once("get_distro_ascii() {\n")
            .context("couldn't find get_distro_ascii function")?;
        let (nf, _) = nf
            .split_once("\n}\n")
            .context("couldn't find end of get_distro_ascii function")?;

        let mut nf = nf.replace('\t', &" ".repeat(4));

        // Remove trailing spaces
        while nf.contains(" \n") {
            nf = nf.replace(" \n", "\n");
        }
        nf
    };

    let case_re = Regex::new(r"case .*? in\n")?;
    let eof_re = Regex::new(r"EOF[ \n]*?;;")?;

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

        Some(AsciiDistro { pattern: pattern.to_owned(), art })
    }
    Ok(blocks.iter().filter_map(|block| parse_block(block)).collect())
}

// Preset parsing
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum PresetEntry {
    Simple(Vec<String>),
    Complex { colors: Vec<String>, weights: Option<Vec<u32>> },
}

type PresetMap = IndexMap<String, PresetEntry>;

fn preset_codegen(json_path: &Path, out_path: &Path) -> Result<()> {
    // 1. Read and parse the JSON file
    let json_str = fs::read_to_string(json_path)?;
    let mut map: PresetMap = serde_json::from_str(&json_str)?;
    let mut f = BufWriter::new(fs::File::create(&out_path)?);

    map.entry("femme".to_string()).or_insert(PresetEntry::Complex {

        colors: vec![
            "#FF1A87".to_string(),
            "#FF6AB1".to_string(),
            "#FFFFFF".to_string(),
            "#9A0731".to_string(),
            "#51091D".to_string(),
        ],
        weights: None,
    });

    // 2. Build the code string
    let mut code_decl = String::new();
    let mut code_match = String::new();
    for (key, data) in map.iter() {
        let colors = match data {
            PresetEntry::Simple(c) => c,
            PresetEntry::Complex { colors, .. } => colors,
        };
        let colors = colors.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
        let uck = key.to_upper_camel_case();

        code_decl += &format!(r#"
            #[serde(rename = "{key}")]
            #[strum(serialize = "{key}")]
            {uck},
        "#);

        let w = if let PresetEntry::Complex { weights: Some(w), .. } = data {
            format!(".and_then(|c| c.with_weights(vec![{}]))", w.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "))
        } else { "".to_string() };

        code_match += &format!(r#"
            Preset::{uck} => ColorProfile::from_hex_colors(vec![{colors}]){w},
        "#);
    }

    // 3. Write the static map to the generated file
    writeln!(f, r#"
    pub use crate::color_profile::ColorProfile;
    use serde::{{Deserialize, Serialize}};
    use strum::{{AsRefStr, EnumCount, EnumString, VariantArray, VariantNames}};

    #[derive(Copy, Clone, Hash, Debug, AsRefStr, Deserialize, EnumCount, EnumString, Serialize, VariantArray, VariantNames)]
    pub enum Preset {{
        {code_decl}
    }}

    impl Preset {{
        pub fn color_profile(&self) -> ColorProfile {{
            (match self {{
                {code_match}
            }})
            .expect("preset color profiles should be valid")
        }}
    }}"#)?;

    f.flush()?;

    Ok(())
}
