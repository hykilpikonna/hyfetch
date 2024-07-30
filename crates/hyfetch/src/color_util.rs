use std::io::{self, Write as _};
use std::num::{ParseFloatError, ParseIntError};
use std::str::FromStr;
use std::sync::OnceLock;

use aho_corasick::AhoCorasick;
use ansi_colours::{ansi256_from_grey, rgb_from_ansi256, AsRGB as _};
use anyhow::{anyhow, Context as _, Result};
use deranged::RangedU8;
use palette::color_difference::ImprovedCiede2000 as _;
use palette::{
    FromColor as _, IntoColor as _, IntoColorMut as _, Lab, LinSrgb, Okhsl, Srgb, SrgbLuma,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{AnsiMode, TerminalTheme};

const MINECRAFT_COLORS: [(&str, &str); 30] = [
    // Minecraft formatting codes
    // ==========================
    ("&0", "\x1b[38;5;0m"),
    ("&1", "\x1b[38;5;4m"),
    ("&2", "\x1b[38;5;2m"),
    ("&3", "\x1b[38;5;6m"),
    ("&4", "\x1b[38;5;1m"),
    ("&5", "\x1b[38;5;5m"),
    ("&6", "\x1b[38;5;3m"),
    ("&7", "\x1b[38;5;7m"),
    ("&8", "\x1b[38;5;8m"),
    ("&9", "\x1b[38;5;12m"),
    ("&a", "\x1b[38;5;10m"),
    ("&b", "\x1b[38;5;14m"),
    ("&c", "\x1b[38;5;9m"),
    ("&d", "\x1b[38;5;13m"),
    ("&e", "\x1b[38;5;11m"),
    ("&f", "\x1b[38;5;15m"),
    ("&l", "\x1b[1m"), // Enable bold text
    ("&o", "\x1b[3m"), // Enable italic text
    ("&n", "\x1b[4m"), // Enable underlined text
    ("&k", "\x1b[8m"), // Enable hidden text
    ("&m", "\x1b[9m"), // Enable strikethrough text
    ("&r", "\x1b[0m"), // Reset everything
    // Extended codes (not officially in Minecraft)
    // ============================================
    ("&-", "\n"),       // Line break
    ("&~", "\x1b[39m"), // Reset text color
    ("&*", "\x1b[49m"), // Reset background color
    ("&L", "\x1b[22m"), // Disable bold text
    ("&O", "\x1b[23m"), // Disable italic text
    ("&N", "\x1b[24m"), // Disable underlined text
    ("&K", "\x1b[28m"), // Disable hidden text
    ("&M", "\x1b[29m"), // Disable strikethrough text
];
const RGB_COLOR_PATTERNS: [&str; 2] = ["&gf(", "&gb("];

/// See https://github.com/mina86/ansi_colours/blob/b9feefce10def2ac632b215ecd20830a4fca7836/src/ansi256.rs#L109
const ANSI256_GRAYSCALE_COLORS: [u8; 30] = [
    16, 59, 102, 145, 188, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244,
    245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255,
];

static MINECRAFT_COLORS_AC: OnceLock<(AhoCorasick, Box<[&str; 30]>)> = OnceLock::new();
static RGB_COLORS_AC: OnceLock<AhoCorasick> = OnceLock::new();

/// Represents the lightness component in [`Okhsl`].
///
/// The range of valid values is
/// [`Lightness::MIN`]`..=`[`Lightness::MAX`]
///
/// [`Okhsl`]: palette::Okhsl
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Lightness(f32);

#[derive(Debug, Error)]
pub enum LightnessError {
    #[error(
        "invalid lightness {0}, expected value between {min} and {max}",
        min = Lightness::MIN,
        max = Lightness::MAX
    )]
    OutOfRange(f32),
}

#[derive(Debug, Error)]
pub enum ParseLightnessError {
    #[error("invalid float")]
    InvalidFloat(#[from] ParseFloatError),
    #[error("invalid lightness")]
    InvalidLightness(#[from] LightnessError),
}

/// An indexed color where the color palette is the set of colors used in
/// neofetch ascii art.
///
/// The range of valid values as supported in neofetch is
/// [`NeofetchAsciiIndexedColor::MIN`]`..=`[`NeofetchAsciiIndexedColor::MAX`]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub struct NeofetchAsciiIndexedColor(
    RangedU8<{ NeofetchAsciiIndexedColor::MIN }, { NeofetchAsciiIndexedColor::MAX }>,
);

/// An indexed color where the color palette is the set of unique colors in a
/// preset.
///
/// The range of valid values depends on the number of unique colors in a
/// certain preset.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub struct PresetIndexedColor(u8);

/// Whether the color is for foreground text or background color.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ForegroundBackground {
    Foreground,
    Background,
}

pub trait ToAnsiString {
    /// Converts RGB to ANSI escape code.
    fn to_ansi_string(&self, mode: AnsiMode, foreground_background: ForegroundBackground)
        -> String;
}

pub trait Theme {
    fn theme(&self) -> TerminalTheme;
}

pub trait ContrastGrayscale {
    /// Calculates the grayscale foreground color which provides the highest
    /// contrast against this background color.
    ///
    /// The returned color is one of the ANSI 256 (8-bit) grayscale colors.
    ///
    /// See <https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg>
    fn contrast_grayscale(&self) -> SrgbLuma<u8>;
}

impl Lightness {
    pub const MAX: f32 = 1.0f32;
    pub const MIN: f32 = 0.0f32;

    pub fn new(value: f32) -> Result<Self, LightnessError> {
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(LightnessError::OutOfRange(value));
        }

        Ok(Self(value))
    }
}

impl TryFrom<f32> for Lightness {
    type Error = LightnessError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Lightness::new(value)
    }
}

impl FromStr for Lightness {
    type Err = ParseLightnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Lightness::new(s.parse()?)?)
    }
}

impl From<Lightness> for f32 {
    fn from(value: Lightness) -> Self {
        value.0
    }
}

impl NeofetchAsciiIndexedColor {
    pub const MAX: u8 = 6;
    pub const MIN: u8 = 1;
}

impl TryFrom<u8> for NeofetchAsciiIndexedColor {
    type Error = deranged::TryFromIntError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

impl FromStr for NeofetchAsciiIndexedColor {
    type Err = deranged::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl From<NeofetchAsciiIndexedColor> for u8 {
    fn from(value: NeofetchAsciiIndexedColor) -> Self {
        value.0.get()
    }
}

impl From<u8> for PresetIndexedColor {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl FromStr for PresetIndexedColor {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl From<PresetIndexedColor> for u8 {
    fn from(value: PresetIndexedColor) -> Self {
        value.0
    }
}

impl ToAnsiString for Srgb<u8> {
    fn to_ansi_string(
        &self,
        mode: AnsiMode,
        foreground_background: ForegroundBackground,
    ) -> String {
        let c: u8 = match foreground_background {
            ForegroundBackground::Foreground => 38,
            ForegroundBackground::Background => 48,
        };
        match mode {
            AnsiMode::Rgb => {
                let [r, g, b]: [u8; 3] = (*self).into();
                format!("\x1b[{c};2;{r};{g};{b}m")
            },
            AnsiMode::Ansi256 => {
                let rgb: [u8; 3] = (*self).into();
                let indexed = rgb.to_ansi256();
                format!("\x1b[{c};5;{indexed}m")
            },
            AnsiMode::Ansi16 => {
                unimplemented!();
            },
        }
    }
}

impl ToAnsiString for SrgbLuma<u8> {
    fn to_ansi_string(
        &self,
        mode: AnsiMode,
        foreground_background: ForegroundBackground,
    ) -> String {
        let c: u8 = match foreground_background {
            ForegroundBackground::Foreground => 38,
            ForegroundBackground::Background => 48,
        };
        match mode {
            AnsiMode::Rgb => {
                let rgb_f32_color: LinSrgb = self.into_linear().into_color();
                let [r, g, b]: [u8; 3] = Srgb::<u8>::from_linear(rgb_f32_color).into();
                format!("\x1b[{c};2;{r};{g};{b}m")
            },
            AnsiMode::Ansi256 => {
                let indexed = ansi256_from_grey(self.luma);
                format!("\x1b[{c};5;{indexed}m")
            },
            AnsiMode::Ansi16 => {
                unimplemented!();
            },
        }
    }
}

impl Theme for Srgb<u8> {
    fn theme(&self) -> TerminalTheme {
        let mut rgb_f32_color: LinSrgb = self.into_linear();

        {
            let okhsl_f32_color: &mut Okhsl = &mut rgb_f32_color.into_color_mut();

            if okhsl_f32_color.lightness > 0.5 {
                TerminalTheme::Light
            } else {
                TerminalTheme::Dark
            }
        }
    }
}

impl ContrastGrayscale for Srgb<u8> {
    fn contrast_grayscale(&self) -> SrgbLuma<u8> {
        let self_lab_f32: Lab = self.into_linear().into_color();

        let mut best_contrast = None;
        for indexed in ANSI256_GRAYSCALE_COLORS {
            let rgb_u8_color: Srgb<u8> = rgb_from_ansi256(indexed).into();
            let lab_f32_color: Lab = rgb_u8_color.into_linear().into_color();
            let diff = lab_f32_color.improved_difference(self_lab_f32);
            best_contrast = match best_contrast {
                Some((_, best_diff)) if diff > best_diff => Some((lab_f32_color, diff)),
                None => Some((lab_f32_color, diff)),
                best => best,
            };
        }
        let (best_lab_f32, _) = best_contrast.expect("`best_contrast` should not be `None`");
        SrgbLuma::from_color(best_lab_f32).into_format()
    }
}

/// Replaces extended minecraft color codes in message.
///
/// Returns message with escape codes.
pub fn color<S>(msg: S, mode: AnsiMode) -> Result<String>
where
    S: AsRef<str>,
{
    let msg = msg.as_ref();

    let msg = {
        let (ac, escape_codes) = MINECRAFT_COLORS_AC.get_or_init(|| {
            let (color_codes, escape_codes): (Vec<_>, Vec<_>) =
                MINECRAFT_COLORS.into_iter().unzip();
            let ac = AhoCorasick::new(color_codes).unwrap();
            (
                ac,
                escape_codes.try_into().expect(
                    "`MINECRAFT_COLORS` should have the same number of elements as \
                     `MINECRAFT_COLORS_AC.get_or_init(...).1`",
                ),
            )
        });
        ac.replace_all(msg, &escape_codes[..])
    };

    let ac = RGB_COLORS_AC.get_or_init(|| AhoCorasick::new(RGB_COLOR_PATTERNS).unwrap());
    let mut dst = String::new();
    let mut ret_err = None;
    ac.replace_all_with(&msg, &mut dst, |m, _, dst| {
        let start = m.end();
        let end = msg[start..]
            .find(')')
            .context("missing closing brace for color code");
        let end = match end {
            Ok(end) => end,
            Err(err) => {
                ret_err = Some(err);
                return false;
            },
        };
        let code = &msg[start..end];
        let foreground_background = if m.pattern().as_usize() == 0 {
            ForegroundBackground::Foreground
        } else {
            ForegroundBackground::Background
        };

        let rgb: Srgb<u8> = if code.starts_with('#') {
            let rgb = code.parse().context("failed to parse hex color");
            match rgb {
                Ok(rgb) => rgb,
                Err(err) => {
                    ret_err = Some(err);
                    return false;
                },
            }
        } else {
            let rgb: Result<[&str; 3], _> = code
                .split(&[',', ';', ' '])
                .filter(|x| x.is_empty())
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| anyhow!("wrong number of rgb components"));
            let rgb = match rgb {
                Ok(rgb) => rgb,
                Err(err) => {
                    ret_err = Some(err);
                    return false;
                },
            };
            let rgb = rgb
                .into_iter()
                .map(u8::from_str)
                .collect::<Result<Vec<_>, _>>()
                .context("failed to parse rgb components");
            let rgb: [u8; 3] = match rgb {
                Ok(rgb) => rgb.try_into().unwrap(),
                Err(err) => {
                    ret_err = Some(err);
                    return false;
                },
            };
            rgb.into()
        };

        dst.push_str(&rgb.to_ansi_string(mode, foreground_background));

        true
    });
    if let Some(err) = ret_err {
        return Err(err);
    }

    Ok(dst)
}

/// Prints with color.
pub fn printc<S>(msg: S, mode: AnsiMode) -> Result<()>
where
    S: AsRef<str>,
{
    writeln!(
        io::stdout(),
        "{msg}",
        msg = color(format!("{msg}&r", msg = msg.as_ref()), mode)
            .context("failed to color message")?
    )
    .context("failed to write message to stdout")
}

/// Clears screen using ANSI escape codes.
pub fn clear_screen(title: Option<&str>, mode: AnsiMode, debug_mode: bool) -> Result<()> {
    if !debug_mode {
        write!(io::stdout(), "\x1b[2J\x1b[H")
            .and_then(|_| io::stdout().flush())
            .context("failed to write clear screen sequence to stdout")?;
    }

    if let Some(title) = title {
        printc(format!("\n{title}\n"), mode).context("failed to print title")?;
    }

    Ok(())
}
