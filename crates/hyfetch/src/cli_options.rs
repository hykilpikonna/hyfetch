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
        .debug_fallback();
    let preset = long("preset")
        .short('p')
        .help(&*format!(
            "Use preset or comma-separated color list or comma-separated hex colors (e.g., \"#ff0000,#00ff00,#0000ff\")
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

    construct!(Options {
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
    })
    .to_options()
    .header(
        &*color(
            "&l&bhyfetch&~&L - neofetch with flags <3",
            AnsiMode::Ansi256,
        )
        .expect("header should not contain invalid color codes"),
    )
    .version(env!("CARGO_PKG_VERSION"))
}

#[cfg(feature = "autocomplete")]
fn complete_preset(input: &String) -> Vec<(String, Option<String>)> {
    <Preset as VariantNames>::VARIANTS
        .iter()
        .chain(iter::once(&"random"))
        .filter_map(|&name| {
            if name.starts_with(input) {
                Some((name.to_owned(), None))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(feature = "autocomplete")]
fn complete_mode(input: &String) -> Vec<(String, Option<String>)> {
    AnsiMode::VARIANTS
        .iter()
        .filter_map(|&name| {
            if name.starts_with(input) {
                Some((name.to_owned(), None))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(feature = "autocomplete")]
fn complete_backend(input: &String) -> Vec<(String, Option<String>)> {
    Backend::VARIANTS
        .iter()
        .filter_map(|&name| {
            if name.starts_with(input) {
                Some((name.to_owned(), None))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_options() {
        options().check_invariants(false)
    }
}
