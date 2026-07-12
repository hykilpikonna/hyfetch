use std::iter;
use std::path::PathBuf;
use std::str::FromStr as _;

use anyhow::Context as _;
#[cfg(feature = "autocomplete")]
use bpaf::ShellComp;
use bpaf::{construct, long, OptionParser, Parser as _};
use directories::BaseDirs;
use itertools::Itertools as _;
use strum::VariantNames;

use crate::color_util::{color, Lightness};
use crate::presets::Preset;
use crate::types::{AnsiMode, Backend};

#[derive(Clone, Debug)]
pub struct Options {
    pub config: bool,
    pub config_file: PathBuf,
    pub preset: Option<String>,
    pub mode: Option<AnsiMode>,
    pub backend: Option<Backend>,
    pub args: Option<Vec<String>>,
    pub scale: Option<f32>,
    pub lightness: Option<Lightness>,
    pub june: bool,
    pub debug: bool,
    pub distro: Option<String>,
    pub ascii_file: Option<PathBuf>,
    pub print_font_logo: bool,
    pub test_print: bool,
    pub ask_exit: bool,
    pub auto_detect_light_dark: Option<bool>,
    #[cfg(feature = "macchina")]
    pub palette_glyph: Option<String>,
    #[cfg(feature = "macchina")]
    pub palette_type: Option<String>,
}

pub fn options() -> OptionParser<Options> {
    let config = long("config").short('c').help("Configure hyfetch").switch();
    let config_file = long("config-file")
        .short('C')
        .help("Use another config file")
        .argument("CONFIG_FILE");
    #[cfg(feature = "autocomplete")]
    let config_file = config_file.complete_shell(ShellComp::Nothing);
    let config_file = config_file
        .fallback_with(|| {
            Ok::<_, anyhow::Error>(
                BaseDirs::new()
                    .context("failed to get base dirs")?
                    .config_dir()
                    .join("hyfetch.json"),
            )
        })
        .format_fallback(|_, f| f.write_str("~/.config/hyfetch.json"));
    let preset = long("preset")
        .short('p')
        .help(&*format!(
            "Use preset or comma-separated color list or comma-separated hex colors (e.g., \"#ff0000,#00ff00,#0000ff\"). Comma-separated preset names will pick one randomly.
PRESET={{{presets}}}",
            presets = <Preset as VariantNames>::VARIANTS
                .iter()
                .chain(iter::once(&"random"))
                .join(",")
        ))
        .argument::<String>("PRESET");
    #[cfg(feature = "autocomplete")]
    let preset = preset.complete(complete_preset);
    let preset = preset.optional();
    let mode = long("mode")
        .short('m')
        .help(&*format!(
            "Color mode
MODE={{{modes}}}",
            modes = AnsiMode::VARIANTS.join(",")
        ))
        .argument::<String>("MODE");
    #[cfg(feature = "autocomplete")]
    let mode = mode.complete(complete_mode);
    let mode = mode
        .parse(|s| {
            AnsiMode::from_str(&s).with_context(|| {
                format!(
                    "MODE should be one of {{{modes}}}",
                    modes = AnsiMode::VARIANTS.join(",")
                )
            })
        })
        .optional();
    let backend = long("backend")
        .short('b')
        .help(&*format!(
            "Choose a *fetch backend
BACKEND={{{backends}}}",
            backends = Backend::VARIANTS.join(",")
        ))
        .argument::<String>("BACKEND");
    #[cfg(feature = "autocomplete")]
    let backend = backend.complete(complete_backend);
    let backend = backend
        .parse(|s| {
            Backend::from_str(&s).with_context(|| {
                format!(
                    "BACKEND should be one of {{{backends}}}",
                    backends = Backend::VARIANTS.join(",")
                )
            })
        })
        .optional();
    let args = long("args")
        .help("Additional arguments pass-through to backend")
        .argument::<String>("ARGS")
        .parse(|s| shell_words::split(&s).context("ARGS should be valid command-line arguments"))
        .optional();
    let scale = long("c-scale")
        .help("Lighten colors by a multiplier")
        .argument("SCALE")
        .optional();
    let lightness = long("c-set-l")
        .help("Set lightness value of the colors")
        .argument("LIGHTNESS")
        .optional();
    let june = long("june").help("Show pride month easter egg").switch();
    let debug = long("debug").help("Debug mode").switch();
    let distro = long("distro")
        .help("Test for a specific distro")
        .argument("DISTRO")
        .optional();
    let test_distro = long("test-distro")
        .help("Test for a specific distro")
        .argument("DISTRO")
        .optional();
    let distro = construct!([distro, test_distro]);
    let ascii_file = long("ascii-file")
        .help("Use a specific file for the ascii art")
        .argument("ASCII_FILE");
    #[cfg(feature = "autocomplete")]
    let ascii_file = ascii_file.complete_shell(ShellComp::Nothing);
    let ascii_file = ascii_file.optional();
    let print_font_logo = long("print-font-logo")
        .help("Print the Font Logo / Nerd Font icon of your distro and exit")
        .switch();
    // hidden
    let test_print = long("test-print")
        .help("Print the ascii distro and exit")
        .switch()
        .hide();
    let ask_exit = long("ask-exit")
        .help("Ask for input before exiting")
        .switch()
        .hide();
    let auto_detect_light_dark = long("auto-detect-light-dark")
        .help("Enables hyfetch to detect light/dark terminal background in runtime")
        .argument("BOOL")
        .optional();

    #[cfg(feature = "macchina")]
    let palette_glyph = long("palette-glyph")
        .help("Sets the glyph to be used for the macchina backend")
        .argument("STR")
        .optional();
    #[cfg(feature = "macchina")]
    let palette_type = long("palette-type")
        .help("Sets the type of palette to be used for the macchina backend")
        .argument("full,light,dark")
        .optional();

    #[cfg(feature = "macchina")]
    return construct!(Options {
        config,
        config_file,
        preset,
        mode,
        backend,
        args,
        scale,
        lightness,
        june,
        debug,
        distro,
        ascii_file,
        print_font_logo,
        // hidden
        test_print,
        ask_exit,
        auto_detect_light_dark,
        palette_glyph,
        palette_type
    })
    .to_options()
    .header(
        &*color(
            "&l&bhyfetch&~&L - neofetch with flags <3",
            AnsiMode::Ansi256,
        )
        .expect("header should not contain invalid color codes"),
    )
    .version(env!("CARGO_PKG_VERSION"));

    #[cfg(not(feature = "macchina"))]
    return construct!(Options {
        config,
        config_file,
        preset,
        mode,
        backend,
        args,
        scale,
        lightness,
        june,
        debug,
        distro,
        ascii_file,
        print_font_logo,
        // hidden
        test_print,
        ask_exit,
        auto_detect_light_dark
    })
    .to_options()
    .header(
        &*color(
            "&l&bhyfetch&~&L - neofetch with flags <3",
            AnsiMode::Ansi256,
        )
        .expect("header should not contain invalid color codes"),
    )
    .version(env!("CARGO_PKG_VERSION"));
}

/// Build a sorted completion list that ranks prefix matches before other substring matches.
///
/// Each entry is `(name, description)` where description is `Some(hint)` when provided.
/// Prefix matches sort before other substring matches; within each group results are sorted
/// by the position of the match and then alphabetically.
#[cfg(feature = "autocomplete")]
fn ranked_completions<'a>(
    candidates: impl Iterator<Item = &'a &'a str>,
    input: &str,
    description: Option<&str>,
) -> Vec<(String, Option<String>)> {
    let desc = description.map(str::to_owned);
    let mut matched: Vec<(bool, usize, &str)> = candidates
        .filter_map(|&name| {
            name.find(input).map(|pos| (name.starts_with(input), pos, name))
        })
        .collect();
    // Prefix matches first (true sorts after false, so negate), then earliest position, then name.
    matched.sort_by_key(|&(is_prefix, pos, name)| (!is_prefix, pos, name));
    matched
        .into_iter()
        .map(|(_, _, name)| (name.to_owned(), desc.clone()))
        .collect()
}

#[cfg(feature = "autocomplete")]
fn complete_preset(input: &String) -> Vec<(String, Option<String>)> {
    let all_variants: Vec<&str> = <Preset as VariantNames>::VARIANTS
        .iter()
        .copied()
        .chain(iter::once("random"))
        .collect();
    ranked_completions(all_variants.iter(), input.as_str(), Some("pride flag preset"))
}

#[cfg(feature = "autocomplete")]
fn complete_mode(input: &String) -> Vec<(String, Option<String>)> {
    let variants: Vec<&str> = AnsiMode::VARIANTS.to_vec();
    ranked_completions(variants.iter(), input.as_str(), Some("color mode"))
}

#[cfg(feature = "autocomplete")]
fn complete_backend(input: &String) -> Vec<(String, Option<String>)> {
    let variants: Vec<&str> = Backend::VARIANTS.to_vec();
    ranked_completions(variants.iter(), input.as_str(), Some("fetch backend"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_options() {
        options().check_invariants(false)
    }

    #[cfg(feature = "autocomplete")]
    #[test]
    fn complete_preset_substring() {
        // "gender" is a substring of "transgender" but not a prefix → must still match
        let results = complete_preset(&"gender".to_owned());
        let names: Vec<&str> = results.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            names.contains(&"transgender"),
            "substring 'gender' should match 'transgender', got: {names:?}"
        );
    }

    #[cfg(feature = "autocomplete")]
    #[test]
    fn complete_preset_prefix_ranked_first() {
        // "trans" is a prefix of "transgender" → it should appear before any non-prefix matches
        let results = complete_preset(&"trans".to_owned());
        assert!(!results.is_empty(), "expected at least one result for 'trans'");
        let first = results[0].0.as_str();
        assert!(
            first.starts_with("trans"),
            "first result should be a prefix match, got: {first}"
        );
    }

    #[cfg(feature = "autocomplete")]
    #[test]
    fn complete_preset_descriptions() {
        // Every completion result should carry a non-empty description
        let results = complete_preset(&"rain".to_owned());
        assert!(!results.is_empty(), "expected at least one result for 'rain'");
        for (name, desc) in &results {
            assert!(
                desc.as_deref().is_some_and(|d| !d.is_empty()),
                "completion for '{name}' should have a description"
            );
        }
    }
}
