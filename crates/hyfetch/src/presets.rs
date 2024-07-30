use std::iter;
use std::num::{NonZeroU8, NonZeroUsize};

use anyhow::{anyhow, Context as _, Result};
use indexmap::IndexSet;
use palette::num::ClampAssign as _;
use palette::{IntoColorMut as _, LinSrgb, Okhsl, Srgb};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumCount, EnumString, VariantArray, VariantNames};
use tracing::debug;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::color_util::{ForegroundBackground, Lightness, ToAnsiString as _};
use crate::types::{AnsiMode, TerminalTheme};

#[derive(
    Copy,
    Clone,
    Hash,
    Debug,
    AsRefStr,
    Deserialize,
    EnumCount,
    EnumString,
    Serialize,
    VariantArray,
    VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Preset {
    Rainbow,

    Transgender,

    Nonbinary,

    Xenogender,

    Agender,

    Queer,

    Genderfluid,

    Bisexual,

    Pansexual,

    Polysexual,

    Omnisexual,

    Omniromantic,

    GayMen,

    Lesbian,

    Abrosexual,

    Asexual,

    Aromantic,

    Aroace1,

    Aroace2,

    Aroace3,

    Greysexual,

    Autosexual,

    Intergender,

    Greygender,

    Akiosexual,

    Bigender,

    Demigender,

    Demiboy,

    Demigirl,

    Transmasculine,

    Transfeminine,

    Genderfaun,

    Demifaun,

    Genderfae,

    Demifae,

    Neutrois,

    Biromantic1,

    Autoromantic,

    Boyflux2,

    Girlflux,

    Genderflux,

    Finsexual,

    Unlabeled1,

    Unlabeled2,

    Pangender,

    #[serde(rename = "gendernonconforming1")]
    #[strum(serialize = "gendernonconforming1")]
    GenderNonconforming1,

    #[serde(rename = "gendernonconforming2")]
    #[strum(serialize = "gendernonconforming2")]
    GenderNonconforming2,

    Femboy,

    Tomboy,

    Gynesexual,

    Androsexual,

    Gendervoid,

    Voidgirl,

    Voidboy,

    NonhumanUnity,

    Plural,

    Fraysexual,

    /// Meme flag
    Beiyang,

    /// Meme flag
    Burger,

    /// Colors from Gilbert Baker's original 1978 flag design
    Baker,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ColorProfile {
    pub colors: Vec<Srgb<u8>>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum AssignLightness {
    Replace(Lightness),
    ClampMax(Lightness),
    ClampMin(Lightness),
}

impl Preset {
    pub fn color_profile(&self) -> ColorProfile {
        (match self {
            Self::Rainbow => ColorProfile::from_hex_colors(vec![
                "#E50000", "#FF8D00", "#FFEE00", "#028121", "#004CFF", "#770088",
            ]),

            Self::Transgender => ColorProfile::from_hex_colors(vec![
                "#55CDFD", "#F6AAB7", "#FFFFFF", "#F6AAB7", "#55CDFD",
            ]),

            Self::Nonbinary => {
                ColorProfile::from_hex_colors(vec!["#FCF431", "#FCFCFC", "#9D59D2", "#282828"])
            },

            // sourced from https://commons.wikimedia.org/wiki/File:Xenogender_pride_flag.svg
            Self::Xenogender => ColorProfile::from_hex_colors(vec![
                "#FF6692", "#FF9A98", "#FFB883", "#FBFFA8", "#85BCFF", "#9D85FF", "#A510FF",
            ]),

            Self::Agender => ColorProfile::from_hex_colors(vec![
                "#000000", "#BABABA", "#FFFFFF", "#BAF484", "#FFFFFF", "#BABABA", "#000000",
            ]),

            Self::Queer => ColorProfile::from_hex_colors(vec!["#B57FDD", "#FFFFFF", "#49821E"]),

            Self::Genderfluid => ColorProfile::from_hex_colors(vec![
                "#FE76A2", "#FFFFFF", "#BF12D7", "#000000", "#303CBE",
            ]),

            Self::Bisexual => ColorProfile::from_hex_colors(vec!["#D60270", "#9B4F96", "#0038A8"]),

            Self::Pansexual => ColorProfile::from_hex_colors(vec!["#FF1C8D", "#FFD700", "#1AB3FF"]),

            Self::Polysexual => {
                ColorProfile::from_hex_colors(vec!["#F714BA", "#01D66A", "#1594F6"])
            },

            // sourced from https://www.flagcolorcodes.com/omnisexual
            Self::Omnisexual => ColorProfile::from_hex_colors(vec![
                "#FE9ACE", "#FF53BF", "#200044", "#6760FE", "#8EA6FF",
            ]),

            Self::Omniromantic => ColorProfile::from_hex_colors(vec![
                "#FEC8E4", "#FDA1DB", "#89739A", "#ABA7FE", "#BFCEFF",
            ]),

            // sourced from https://www.flagcolorcodes.com/gay-men
            Self::GayMen => ColorProfile::from_hex_colors(vec![
                "#078D70", "#98E8C1", "#FFFFFF", "#7BADE2", "#3D1A78",
            ]),

            Self::Lesbian => ColorProfile::from_hex_colors(vec![
                "#D62800", "#FF9B56", "#FFFFFF", "#D462A6", "#A40062",
            ]),

            // used colorpicker to source from https://fyeahaltpride.tumblr.com/post/151704251345/could-you-guys-possibly-make-an-abrosexual-pride
            Self::Abrosexual => ColorProfile::from_hex_colors(vec![
                "#46D294", "#A3E9CA", "#FFFFFF", "#F78BB3", "#EE1766",
            ]),

            Self::Asexual => {
                ColorProfile::from_hex_colors(vec!["#000000", "#A4A4A4", "#FFFFFF", "#810081"])
            },

            Self::Aromantic => ColorProfile::from_hex_colors(vec![
                "#3BA740", "#A8D47A", "#FFFFFF", "#ABABAB", "#000000",
            ]),

            // sourced from https://flag.library.lgbt/flags/aroace/
            Self::Aroace1 => ColorProfile::from_hex_colors(vec![
                "#E28C00", "#ECCD00", "#FFFFFF", "#62AEDC", "#203856",
            ]),

            // sourced from https://flag.library.lgbt/flags/aroace/
            Self::Aroace2 => ColorProfile::from_hex_colors(vec![
                "#000000", "#810081", "#A4A4A4", "#FFFFFF", "#A8D47A", "#3BA740",
            ]),

            // sourced from https://flag.library.lgbt/flags/aroace/
            Self::Aroace3 => ColorProfile::from_hex_colors(vec![
                "#3BA740", "#A8D47A", "#FFFFFF", "#ABABAB", "#000000", "#A4A4A4", "#FFFFFF",
                "#810081",
            ]),

            // sourced from https://www.flagcolorcodes.com/greysexual
            Self::Greysexual => ColorProfile::from_hex_colors(vec![
                "#740194", "#AEB1AA", "#FFFFFF", "#AEB1AA", "#740194",
            ]),

            // sourced from https://www.flagcolorcodes.com/autosexual
            Self::Autosexual => ColorProfile::from_hex_colors(vec!["#99D9EA", "#7F7F7F"]),

            // sourced from https://www.flagcolorcodes.com/intergender
            Self::Intergender => {
                ColorProfile::from_hex_colors(vec!["#900DC2", "#FFE54F", "#900DC2"])
                    .and_then(|c| c.with_weights(vec![2, 1, 2]))
            },

            // sourced from https://www.flagcolorcodes.com/greygender
            Self::Greygender => ColorProfile::from_hex_colors(vec![
                "#B3B3B3", "#FFFFFF", "#062383", "#FFFFFF", "#535353",
            ])
            .and_then(|c| c.with_weights(vec![2, 1, 2, 1, 2])),

            // sourced from https://www.flagcolorcodes.com/akiosexual
            Self::Akiosexual => ColorProfile::from_hex_colors(vec![
                "#F9485E", "#FEA06A", "#FEF44C", "#FFFFFF", "#000000",
            ]),

            // sourced from https://www.flagcolorcodes.com/bigender
            Self::Bigender => ColorProfile::from_hex_colors(vec![
                "#C479A2", "#EDA5CD", "#D6C7E8", "#FFFFFF", "#D6C7E8", "#9AC7E8", "#6D82D1",
            ]),

            // yellow sourced from https://lgbtqia.fandom.com/f/p/4400000000000041031
            // other colors sourced from demiboy and demigirl flags
            Self::Demigender => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#FBFF75", "#FFFFFF", "#FBFF75", "#C4C4C4", "#7F7F7F",
            ]),

            // sourced from https://www.flagcolorcodes.com/demiboy
            Self::Demiboy => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#9DD7EA", "#FFFFFF", "#9DD7EA", "#C4C4C4", "#7F7F7F",
            ]),

            // sourced from https://www.flagcolorcodes.com/demigirl
            Self::Demigirl => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#FDADC8", "#FFFFFF", "#FDADC8", "#C4C4C4", "#7F7F7F",
            ]),

            // sourced from https://www.flagcolorcodes.com/transmasculine
            Self::Transmasculine => ColorProfile::from_hex_colors(vec![
                "#FF8ABD", "#CDF5FE", "#9AEBFF", "#74DFFF", "#9AEBFF", "#CDF5FE", "#FF8ABD",
            ]),

            // used colorpicker to source from https://www.deviantart.com/pride-flags/art/Trans-Woman-Transfeminine-1-543925985
            // linked from https://gender.fandom.com/wiki/Transfeminine
            Self::Transfeminine => ColorProfile::from_hex_colors(vec![
                "#73DEFF", "#FFE2EE", "#FFB5D6", "#FF8DC0", "#FFB5D6", "#FFE2EE", "#73DEFF",
            ]),

            // sourced from https://www.flagcolorcodes.com/genderfaun
            Self::Genderfaun => ColorProfile::from_hex_colors(vec![
                "#FCD689", "#FFF09B", "#FAF9CD", "#FFFFFF", "#8EDED9", "#8CACDE", "#9782EC",
            ]),

            // sourced from https://www.flagcolorcodes.com/demifaun
            Self::Demifaun => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C6C6C6", "#FCC688", "#FFF19C", "#FFFFFF", "#8DE0D5", "#9682EC",
                "#C6C6C6", "#7F7F7F",
            ])
            .and_then(|c| c.with_weights(vec![2, 2, 1, 1, 1, 1, 1, 2, 2])),

            // sourced from https://www.flagcolorcodes.com/genderfae
            Self::Genderfae => ColorProfile::from_hex_colors(vec![
                "#97C3A5", "#C3DEAE", "#F9FACD", "#FFFFFF", "#FCA2C4", "#DB8AE4", "#A97EDD",
            ]),

            // used colorpicker to source form https://www.deviantart.com/pride-flags/art/Demifae-870194777
            Self::Demifae => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C5C5C5", "#97C3A4", "#C4DEAE", "#FFFFFF", "#FCA2C5", "#AB7EDF",
                "#C5C5C5", "#7F7F7F",
            ])
            .and_then(|c| c.with_weights(vec![2, 2, 1, 1, 1, 1, 1, 2, 2])),

            // sourced from https://www.flagcolorcodes.com/neutrois
            Self::Neutrois => ColorProfile::from_hex_colors(vec!["#FFFFFF", "#1F9F00", "#000000"]),

            // sourced from https://www.flagcolorcodes.com/biromantic-alternate-2
            Self::Biromantic1 => ColorProfile::from_hex_colors(vec![
                "#8869A5", "#D8A7D8", "#FFFFFF", "#FDB18D", "#151638",
            ]),

            // sourced from https://www.flagcolorcodes.com/autoromantic
            Self::Autoromantic => ColorProfile::from_hex_colors(
                // symbol interpreted
                vec!["#99D9EA", "#3DA542", "#7F7F7F"],
            )
            .and_then(|c| c.with_weights(vec![2, 1, 2])),

            // sourced from https://www.flagcolorcodes.com/boyflux-alternate-2
            Self::Boyflux2 => ColorProfile::from_hex_colors(vec![
                "#E48AE4", "#9A81B4", "#55BFAB", "#FFFFFF", "#A8A8A8", "#81D5EF", "#69ABE5",
                "#5276D4",
            ])
            .and_then(|c| c.with_weights(vec![1, 1, 1, 1, 1, 5, 5, 5])),

            // sourced from https://commons.wikimedia.org/wiki/File:Girlflux_Pride_Flag.jpg
            Self::Girlflux => ColorProfile::from_hex_colors(vec![
                "f9e6d7", "f2526c", "bf0311", "e9c587", "bf0311", "f2526c", "f9e6d7",
            ]),

            // sourced from https://www.deviantart.com/pride-flags/art/Genderflux-1-543925589
            Self::Genderflux => ColorProfile::from_hex_colors(vec![
                "f47694", "f2a2b9", "cecece", "7ce0f7", "3ecdf9", "fff48d",
            ]),

            // sourced from https://lgbtqia.wiki/wiki/Finsexual
            Self::Finsexual => ColorProfile::from_hex_colors(vec![
                "#B18EDF", "#D7B1E2", "#F7CDE9", "#F39FCE", "#EA7BB3",
            ]),

            // sourced from https://web.archive.org/web/20221002181913/https://unlabeledinfo.carrd.co/#flags
            Self::Unlabeled1 => {
                ColorProfile::from_hex_colors(vec!["#EAF8E4", "#FDFDFB", "#E1EFF7", "#F4E2C4"])
            },

            // sourced from https://web.archive.org/web/20221002181913/https://unlabeledinfo.carrd.co/#flags
            Self::Unlabeled2 => ColorProfile::from_hex_colors(vec![
                "#250548", "#FFFFFF", "#F7DCDA", "#EC9BEE", "#9541FA", "#7D2557",
            ]),

            Self::Pangender => ColorProfile::from_hex_colors(vec![
                "#FFF798", "#FEDDCD", "#FFEBFB", "#FFFFFF", "#FFEBFB", "#FEDDCD", "#FFF798",
            ]),

            Self::GenderNonconforming1 => ColorProfile::from_hex_colors(vec![
                "#50284d", "#96467b", "#5c96f7", "#ffe6f7", "#5c96f7", "#96467b", "#50284d",
            ])
            .and_then(|c| c.with_weights(vec![4, 1, 1, 1, 1, 1, 4])),

            Self::GenderNonconforming2 => ColorProfile::from_hex_colors(vec![
                "#50284d", "#96467b", "#5c96f7", "#ffe6f7", "#5c96f7", "#96467b", "#50284d",
            ]),

            Self::Femboy => ColorProfile::from_hex_colors(vec![
                "#d260a5", "#e4afcd", "#fefefe", "#57cef8", "#fefefe", "#e4afcd", "#d260a5",
            ]),

            Self::Tomboy => ColorProfile::from_hex_colors(vec![
                "#2f3fb9", "#613a03", "#fefefe", "#f1a9b7", "#fefefe", "#613a03", "#2f3fb9",
            ]),

            // sourced from https://lgbtqia.fandom.com/wiki/Gynesexual
            Self::Gynesexual => {
                ColorProfile::from_hex_colors(vec!["#F4A9B7", "#903F2B", "#5B953B"])
            },

            // sourced from https://lgbtqia.fandom.com/wiki/Androsexual
            Self::Androsexual => {
                ColorProfile::from_hex_colors(vec!["#01CCFF", "#603524", "#B799DE"])
            },

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Gendervoid => ColorProfile::from_hex_colors(vec![
                "#081149", "#4B484B", "#000000", "#4B484B", "#081149",
            ]),

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Voidgirl => ColorProfile::from_hex_colors(vec![
                "#180827", "#7A5A8B", "#E09BED", "#7A5A8B", "#180827",
            ]),

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Voidboy => ColorProfile::from_hex_colors(vec![
                "#0B130C", "#547655", "#66B969", "#547655", "#0B130C",
            ]),

            // used https://twitter.com/foxbrained/status/1667621855518236674/photo/1 as source and colorpicked
            Self::NonhumanUnity => {
                ColorProfile::from_hex_colors(vec!["#177B49", "#FFFFFF", "#593C90"])
            },

            // used https://pluralpedia.org/w/Plurality#/media/File:Plural-Flag-1.jpg as source and colorpicked
            Self::Plural => ColorProfile::from_hex_colors(vec![
                "#2D0625", "#543475", "#7675C3", "#89C7B0", "#F3EDBD",
            ]),

            // sampled from https://es.m.wikipedia.org/wiki/Archivo:Fraysexual_flag.jpg
            Self::Fraysexual => {
                ColorProfile::from_hex_colors(vec!["#226CB5", "#94E7DD", "#FFFFFF", "#636363"])
            },

            Self::Beiyang => ColorProfile::from_hex_colors(vec![
                "#DF1B12", "#FFC600", "#01639D", "#FFFFFF", "#000000",
            ]),

            Self::Burger => ColorProfile::from_hex_colors(vec![
                "#F3A26A", "#498701", "#FD1C13", "#7D3829", "#F3A26A",
            ]),

            // used https://gilbertbaker.com/rainbow-flag-color-meanings/ as source and colorpicked
            Self::Baker => ColorProfile::from_hex_colors(vec![
                "#F23D9E", "#F80A24", "#F78022", "#F9E81F", "#1E972E", "#1B86BC", "#243897",
                "#6F0A82",
            ]),
        })
        .expect("preset color profiles should be valid")
    }
}

impl ColorProfile {
    pub fn new(colors: Vec<Srgb<u8>>) -> Self {
        Self { colors }
    }

    pub fn from_hex_colors<S>(hex_colors: Vec<S>) -> Result<Self>
    where
        S: AsRef<str>,
    {
        let colors = hex_colors
            .into_iter()
            .map(|s| s.as_ref().parse())
            .collect::<Result<_, _>>()
            .context("failed to parse hex colors")?;
        Ok(Self::new(colors))
    }

    /// Maps colors based on weights.
    ///
    /// # Arguments
    ///
    /// * `weights` - Weights of each color (`weights[i]` = how many times
    ///   `colors[i]` appears)
    pub fn with_weights(&self, weights: Vec<u8>) -> Result<Self> {
        if weights.len() != self.colors.len() {
            debug!(?weights, ?self.colors, "length mismatch between `weights` and `colors`");
            return Err(anyhow!(
                "`weights` should have the same number of elements as `colors`"
            ));
        }

        let mut weighted_colors = Vec::new();

        for (i, w) in weights.into_iter().enumerate() {
            weighted_colors.extend(iter::repeat(self.colors[i]).take(usize::from(w)));
        }

        Ok(Self::new(weighted_colors))
    }

    /// Creates a new color profile, with the colors spread to the specified
    /// length.
    pub fn with_length(&self, length: NonZeroU8) -> Result<Self> {
        let orig_len = self.colors.len();
        let orig_len: NonZeroUsize = orig_len.try_into().expect("`colors` should not be empty");
        let orig_len: NonZeroU8 = orig_len
            .try_into()
            .expect("`colors` should not have more than 255 elements");
        // TODO: I believe weird things can happen because of this...
        // if length < orig_len {
        //     unimplemented!("compressing length of color profile not implemented");
        // }
        let center_i = usize::from(orig_len.get() / 2);

        // How many copies of each color should be displayed at least?
        let repeats = length.get().div_euclid(orig_len.get());
        let mut weights = vec![repeats; NonZeroUsize::from(orig_len).get()];

        // How many extra spaces left?
        let mut extras = length.get().rem_euclid(orig_len.get());

        // If there is an odd space left, extend the center by one space
        if extras % 2 == 1 {
            weights[center_i] = weights[center_i].checked_add(1).unwrap();
            extras = extras.checked_sub(1).unwrap();
        }

        // Add weight to border until there's no space left (extras must be even at this
        // point)
        let weights_len = weights.len();
        for border_i in 0..usize::from(extras / 2) {
            weights[border_i] = weights[border_i].checked_add(1).unwrap();
            let border_opp = weights_len
                .checked_sub(border_i)
                .unwrap()
                .checked_sub(1)
                .unwrap();
            weights[border_opp] = weights[border_opp].checked_add(1).unwrap();
        }

        self.with_weights(weights)
    }

    /// Colors a text.
    ///
    /// # Arguments
    ///
    /// * `foreground_background` - Whether the color is shown on the foreground
    ///   text or the background block
    /// * `space_only` - Whether to only color spaces
    pub fn color_text<S>(
        &self,
        txt: S,
        color_mode: AnsiMode,
        foreground_background: ForegroundBackground,
        space_only: bool,
    ) -> Result<String>
    where
        S: AsRef<str>,
    {
        let txt = txt.as_ref();

        let txt: Vec<&str> = txt.graphemes(true).collect();

        let ColorProfile { colors } = {
            let length = txt.len();
            let length: NonZeroUsize = length.try_into().context("`txt` should not be empty")?;
            let length: NonZeroU8 = length.try_into().with_context(|| {
                format!(
                    "`txt` should not have more than {limit} characters",
                    limit = u8::MAX
                )
            })?;
            self.with_length(length)
                .with_context(|| format!("failed to spread color profile to length {length}"))?
        };

        let mut buf = String::new();
        for (i, &gr) in txt.iter().enumerate() {
            if space_only && gr != " " {
                if i > 0 && txt[i.checked_sub(1).unwrap()] == " " {
                    buf.push_str("\x1b[39;49m");
                }
                buf.push_str(gr);
            } else {
                buf.push_str(&colors[i].to_ansi_string(color_mode, foreground_background));
                buf.push_str(gr);
            }
        }

        buf.push_str("\x1b[39;49m");
        Ok(buf)
    }

    /// Creates a new color profile, with the colors lightened by a multiplier.
    pub fn lighten(&self, multiplier: f32) -> Self {
        let mut rgb_f32_colors: Vec<LinSrgb> =
            self.colors.iter().map(|c| c.into_linear()).collect();

        {
            let okhsl_f32_colors: &mut [Okhsl] = &mut rgb_f32_colors.into_color_mut();

            for okhsl_f32_color in okhsl_f32_colors {
                okhsl_f32_color.lightness *= multiplier;
            }
        }

        let rgb_u8_colors: Vec<_> = rgb_f32_colors
            .into_iter()
            .map(Srgb::<u8>::from_linear)
            .collect();

        Self {
            colors: rgb_u8_colors,
        }
    }

    /// Creates a new color profile, with the colors set to the specified
    /// [`Okhsl`] lightness value.
    pub fn with_lightness(&self, assign_lightness: AssignLightness) -> Self {
        let mut rgb_f32_colors: Vec<LinSrgb> =
            self.colors.iter().map(|c| c.into_linear()).collect();

        {
            let okhsl_f32_colors: &mut [Okhsl] = &mut rgb_f32_colors.into_color_mut();

            for okhsl_f32_color in okhsl_f32_colors {
                match assign_lightness {
                    AssignLightness::Replace(lightness) => {
                        okhsl_f32_color.lightness = lightness.into();
                    },
                    AssignLightness::ClampMax(lightness) => {
                        okhsl_f32_color.lightness.clamp_max_assign(lightness.into());
                    },
                    AssignLightness::ClampMin(lightness) => {
                        okhsl_f32_color.lightness.clamp_min_assign(lightness.into());
                    },
                }
            }
        }

        let rgb_u8_colors: Vec<Srgb<u8>> = rgb_f32_colors
            .into_iter()
            .map(Srgb::<u8>::from_linear)
            .collect();

        Self {
            colors: rgb_u8_colors,
        }
    }

    /// Creates a new color profile, with the colors set to the specified
    /// [`Okhsl`] lightness value, adapted to the terminal theme.
    pub fn with_lightness_adaptive(&self, lightness: Lightness, theme: TerminalTheme) -> Self {
        match theme {
            TerminalTheme::Dark => self.with_lightness(AssignLightness::ClampMin(lightness)),
            TerminalTheme::Light => self.with_lightness(AssignLightness::ClampMax(lightness)),
        }
    }

    /// Creates another color profile with only the unique colors.
    pub fn unique_colors(&self) -> Self {
        let unique_colors: IndexSet<[u8; 3]> = self.colors.iter().map(|&c| c.into()).collect();
        let unique_colors: Vec<Srgb<u8>> = unique_colors.into_iter().map(|c| c.into()).collect();
        Self::new(unique_colors)
    }
}
