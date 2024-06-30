use serde::{Deserialize, Serialize};
use strum::{EnumString, VariantNames};

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize, EnumString, Serialize, VariantNames)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum AnsiMode {
    #[serde(rename = "8bit")]
    #[strum(serialize = "8bit")]
    Ansi256,
    Rgb,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LightDark {
    Light,
    Dark,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize, EnumString, Serialize, VariantNames)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Backend {
    Qwqfetch,
    Neofetch,
    Fastfetch,
    FastfetchOld,
}
