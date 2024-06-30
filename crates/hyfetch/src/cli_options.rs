use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
#[cfg(feature = "autocomplete")]
use bpaf::ShellComp;
use bpaf::{construct, long, OptionParser, Parser};
use directories::BaseDirs;
use strum::VariantNames;

use crate::presets::Preset;
use crate::types::{AnsiMode, Backend};

#[derive(Clone, Debug)]
pub struct Options {
    pub config: bool,
    pub config_file: PathBuf,
    pub preset: Option<Preset>,
    pub mode: Option<AnsiMode>,
    pub backend: Option<Backend>,
    pub args: Vec<String>,
    pub colors_scale: Option<f32>,
    pub colors_set_lightness: Option<f32>,
    pub colors_use_overlay: bool,
    pub june: bool,
    pub debug: bool,
    pub distro: Option<String>,
    pub ascii_file: Option<PathBuf>,
    pub test_print: bool,
    pub ask_exit: bool,
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
                    .context("Failed to get base dirs")?
                    .config_dir()
                    .join("hyfetch.json"),
            )
        })
        .debug_fallback();
    let preset = long("preset")
        .short('p')
        .help(&*format!(
            "Use preset
PRESET={{{}}}",
            Preset::VARIANTS.join(",")
        ))
        .argument("PRESET");
    #[cfg(feature = "autocomplete")]
    let preset = preset.complete(complete_preset);
    let preset = preset
        .parse(|s| {
            Preset::from_str(&s).with_context(|| {
                format!("PRESET should be one of {{{}}}", Preset::VARIANTS.join(","))
            })
        })
        .optional();
    let mode = long("mode")
        .short('m')
        .help(&*format!(
            "Color mode
MODE={{{}}}",
            AnsiMode::VARIANTS.join(",")
        ))
        .argument("MODE");
    #[cfg(feature = "autocomplete")]
    let mode = mode.complete(complete_mode);
    let mode = mode
        .parse(|s| {
            AnsiMode::from_str(&s).with_context(|| {
                format!("MODE should be one of {{{}}}", AnsiMode::VARIANTS.join(","))
            })
        })
        .optional();
    let backend = long("backend")
        .short('b')
        .help(&*format!(
            "Choose a *fetch backend
BACKEND={{{}}}",
            Backend::VARIANTS.join(",")
        ))
        .argument("BACKEND");
    #[cfg(feature = "autocomplete")]
    let backend = backend.complete(complete_backend);
    let backend = backend
        .parse(|s| {
            Backend::from_str(&s).with_context(|| {
                format!(
                    "BACKEND should be one of {{{}}}",
                    Backend::VARIANTS.join(",")
                )
            })
        })
        .optional();
    let args = long("args")
        .help("Additional arguments pass-through to backend")
        .argument::<String>("ARGS")
        .parse(|s| shell_words::split(&s).context("ARGS should be valid command-line arguments"))
        .fallback(vec![]);
    let colors_scale = long("c-scale")
        .help("Lighten colors by a multiplier")
        .argument("SCALE")
        .optional();
    let colors_set_lightness = long("c-set-l")
        .help("Set lightness value of the colors")
        .argument("LIGHT")
        .optional();
    let colors_use_overlay = long("c-overlay")
        .help("Use experimental overlay color adjusting instead of HSL lightness")
        .switch();
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
    let test_print = long("test-print")
        .help("Print the ascii distro and exit")
        .switch()
        .hide();
    let ask_exit = long("ask-exit")
        .help("Ask for input before exiting")
        .switch()
        .hide();

    construct!(Options {
        config,
        config_file,
        preset,
        mode,
        backend,
        args,
        colors_scale,
        colors_set_lightness,
        colors_use_overlay,
        june,
        debug,
        distro,
        ascii_file,
        // hidden
        test_print,
        ask_exit,
    })
    .to_options()
    .version(env!("CARGO_PKG_VERSION"))
}

#[cfg(feature = "autocomplete")]
fn complete_preset(input: &String) -> Vec<(String, Option<String>)> {
    Preset::VARIANTS
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
