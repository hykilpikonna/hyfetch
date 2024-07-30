use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumString, VariantNames};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, AsRefStr, Deserialize, EnumString, Serialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum AnsiMode {
    #[serde(rename = "ansi")]
    #[serde(skip)]
    #[strum(serialize = "ansi")]
    #[strum(disabled)]
    Ansi16,
    #[serde(rename = "8bit")]
    #[strum(serialize = "8bit")]
    Ansi256,
    Rgb,
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Hash,
    Debug,
    AsRefStr,
    Deserialize,
    EnumString,
    Serialize,
    VariantNames,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TerminalTheme {
    Light,
    Dark,
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Hash,
    Debug,
    AsRefStr,
    Deserialize,
    EnumString,
    Serialize,
    VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Backend {
    Neofetch,
    Fastfetch,
    #[cfg(feature = "macchina")]
    Macchina,
}

// See https://github.com/Peternator7/strum/issues/244
impl VariantNames for AnsiMode {
    const VARIANTS: &'static [&'static str] = &["8bit", "rgb"];
}
