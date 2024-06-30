use serde::{Deserialize, Serialize};

use crate::neofetch_util::ColorAlignment;
use crate::presets::Preset;
use crate::types::{AnsiMode, Backend, LightDark};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub preset: Preset,
    pub mode: AnsiMode,
    pub light_dark: LightDark,
    pub lightness: Option<f32>,
    pub color_align: ColorAlignment,
    pub backend: Backend,
    #[serde(with = "self::args_serde_with")]
    pub args: Vec<String>,
    pub distro: Option<String>,
    pub pride_month_disable: bool,
}

mod args_serde_with {
    use std::fmt;

    use serde::de::{self, value, Deserialize, Deserializer, SeqAccess, Visitor};
    use serde::ser::Serializer;

    pub(super) fn serialize<S>(value: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&shell_words::join(value))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrVec;

        impl<'de> Visitor<'de> for StringOrVec {
            type Value = Vec<String>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or list of strings")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(vec![])
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                shell_words::split(s).map_err(de::Error::custom)
            }

            fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
            where
                S: SeqAccess<'de>,
            {
                Deserialize::deserialize(value::SeqAccessDeserializer::new(seq))
            }
        }

        deserializer.deserialize_any(StringOrVec)
    }
}
