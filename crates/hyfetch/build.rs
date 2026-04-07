use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use fs_extra::dir::CopyOptions;
use heck::ToUpperCamelCase;
use indexmap::IndexMap;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization as _;

#[derive(Debug)]
struct AsciiDistro {
    pattern: String,
    color: String,
    foreground: Vec<u8>,
    background: Option<u8>,
    art: String,
}

#[derive(Deserialize, Debug)]
struct DistroHeader {
    #[serde(rename = "match")]
    pattern: String,
    color: serde_json::Value,
    foreground: Option<Vec<u8>>,
    background: Option<u8>,
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

    let data_dir = anything_that_exist(&[
        &dir.join("hyfetch/data"),
        &dir.join("../../hyfetch/data"),
    ]).context("couldn't find hyfetch/data")?;
    
    let dst_root = o.join("hyfetch");
    fs::create_dir_all(&dst_root)?;
    
    // Copy hyfetch/data
    let opt = CopyOptions { overwrite: true, copy_inside: true, ..CopyOptions::default() };
    fs_extra::dir::copy(&data_dir, &dst_root, &opt)?;

    // Copy neofetch
    let neofetch_src = anything_that_exist(&[
        &dir.join("neofetch"),
        &dir.join("../../neofetch"),
    ]).context("couldn't find neofetch")?;
    fs::copy(&neofetch_src, o.join("neofetch"))?;

    preset_codegen(&o.join("hyfetch/data/presets.json"), &o.join("presets.rs"))?;
    
    let distros_dir = data_dir.join("distros");
    export_distros(&distros_dir, &o)?;
    Ok(())
}

fn export_distros(distro_dir: &Path, out_path: &Path) -> Result<()>
{
    let distros = parse_ascii_distros(distro_dir)?;
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

    pub fn color(&self) -> &str {
        match self {
"###,
    );

    for (variant, AsciiDistro { color, .. }) in &variants {
        write!(buf, r###"
            Self::{variant} => {color:?},
"###, color = color)?;
    }

    buf.push_str(
        r###"
        }
    }

    pub fn foreground(&self) -> &[u8] {
        match self {
"###,
    );

    for (variant, AsciiDistro { foreground, .. }) in &variants {
        if foreground.is_empty() {
            write!(buf, r###"
            Self::{variant} => &[],
"###)?;
        } else {
            write!(buf, r###"
            Self::{variant} => &{:?},
"###, foreground)?;
        }
    }

    buf.push_str(
        r###"
        }
    }

    pub fn background(&self) -> Option<u8> {
        match self {
"###,
    );

    for (variant, AsciiDistro { background, .. }) in &variants {
        if let Some(b) = background {
            write!(buf, r###"
            Self::{variant} => Some({b}),
"###)?;
        } else {
            write!(buf, r###"
            Self::{variant} => None,
"###)?;
        }
    }

    buf.push_str(
        r###"
        }
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

fn parse_ascii_distros(distro_dir: &Path) -> Result<Vec<AsciiDistro>>
{
    let mut distros = Vec::new();
    let mut paths: Vec<_> = fs::read_dir(distro_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    paths.sort();

    for path in paths {
        if path.extension().and_then(|s| s.to_str()) == Some("ascii") {
            let content = fs::read_to_string(&path)?;
            let (header_line, art) = content.split_once('\n').context("invalid distro file")?;
            let header: DistroHeader = serde_json::from_str(header_line)?;
            let color = match header.color {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                _ => "7".to_owned(),
            };
            distros.push(AsciiDistro {
                pattern: header.pattern,
                color,
                foreground: header.foreground.unwrap_or_default(),
                background: header.background,
                art: art.to_owned(),
            });
        }
    }
    Ok(distros)
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
    let map: PresetMap = serde_json::from_str(&json_str)?;
    let mut f = BufWriter::new(fs::File::create(&out_path)?);

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
