use std::iter;

use anyhow::{anyhow, Context, Result};
use indexmap::IndexSet;
use rgb::RGB8;
use serde::{Deserialize, Serialize};
use strum::{EnumString, VariantNames};

use crate::color_util::FromHex;

#[derive(Clone, Hash, Debug, Deserialize, EnumString, Serialize, VariantNames)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Preset {
    Abrosexual,
    Agender,
    Akiosexual,
    Androsexual,
    Aroace1,
    Aroace2,
    Aroace3,
    Aromantic,
    Asexual,
    Autoromantic,
    Autosexual,
    /// Colors from Gilbert Baker's original 1978 flag design
    Baker,
    /// Meme flag
    Beiyang,
    Bigender,
    Biromantic1,
    Bisexual,
    Boyflux2,
    /// Meme flag
    Burger,
    Demiboy,
    Demifae,
    Demifaun,
    Demigender,
    Demigirl,
    Femboy,
    Finsexual,
    Fraysexual,
    GayMen,
    Genderfae,
    Genderfaun,
    Genderfluid,
    Genderflux,
    #[serde(rename = "gendernonconforming1")]
    #[strum(serialize = "gendernonconforming1")]
    GenderNonconforming1,
    #[serde(rename = "gendernonconforming2")]
    #[strum(serialize = "gendernonconforming2")]
    GenderNonconforming2,
    Gendervoid,
    Girlflux,
    Greygender,
    #[serde(alias = "biromantic2")]
    Greysexual,
    Gynesexual,
    Intergender,
    Lesbian,
    Neutrois,
    Nonbinary,
    NonhumanUnity,
    Omniromantic,
    Omnisexual,
    Pangender,
    Pansexual,
    Plural,
    Polysexual,
    Queer,
    Rainbow,
    Tomboy,
    Transfeminine,
    Transgender,
    Transmasculine,
    Unlabeled1,
    Unlabeled2,
    Voidboy,
    Voidgirl,
    Xenogender,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ColorProfile {
    pub colors: Vec<RGB8>,
}

impl Preset {
    pub fn color_profile(&self) -> ColorProfile {
        (match self {
            // used colorpicker to source from https://fyeahaltpride.tumblr.com/post/151704251345/could-you-guys-possibly-make-an-abrosexual-pride
            Self::Abrosexual => ColorProfile::from_hex_colors(vec![
                "#46D294", "#A3E9CA", "#FFFFFF", "#F78BB3", "#EE1766",
            ]),

            Self::Agender => ColorProfile::from_hex_colors(vec![
                "#000000", "#BABABA", "#FFFFFF", "#BAF484", "#FFFFFF", "#BABABA", "#000000",
            ]),

            // sourced from https://www.flagcolorcodes.com/akiosexual
            Self::Akiosexual => ColorProfile::from_hex_colors(vec![
                "#F9485E", "#FEA06A", "#FEF44C", "#FFFFFF", "#000000",
            ]),

            // sourced from https://lgbtqia.fandom.com/wiki/Androsexual
            Self::Androsexual => {
                ColorProfile::from_hex_colors(vec!["#01CCFF", "#603524", "#B799DE"])
            },

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

            Self::Aromantic => ColorProfile::from_hex_colors(vec![
                "#3BA740", "#A8D47A", "#FFFFFF", "#ABABAB", "#000000",
            ]),

            Self::Asexual => {
                ColorProfile::from_hex_colors(vec!["#000000", "#A4A4A4", "#FFFFFF", "#810081"])
            },

            // sourced from https://www.flagcolorcodes.com/autoromantic
            Self::Autoromantic => ColorProfile::from_hex_colors(
                // symbol interpreted
                vec!["#99D9EA", "#99D9EA", "#3DA542", "#7F7F7F", "#7F7F7F"],
            ),

            // sourced from https://www.flagcolorcodes.com/autosexual
            Self::Autosexual => ColorProfile::from_hex_colors(vec!["#99D9EA", "#7F7F7F"]),

            // used https://gilbertbaker.com/rainbow-flag-color-meanings/ as source and colorpicked
            Self::Baker => ColorProfile::from_hex_colors(vec![
                "#F23D9E", "#F80A24", "#F78022", "#F9E81F", "#1E972E", "#1B86BC", "#243897",
                "#6F0A82",
            ]),

            Self::Beiyang => ColorProfile::from_hex_colors(vec![
                "#DF1B12", "#FFC600", "#01639D", "#FFFFFF", "#000000",
            ]),

            // sourced from https://www.flagcolorcodes.com/bigender
            Self::Bigender => ColorProfile::from_hex_colors(vec![
                "#C479A2", "#EDA5CD", "#D6C7E8", "#FFFFFF", "#D6C7E8", "#9AC7E8", "#6D82D1",
            ]),

            // sourced from https://www.flagcolorcodes.com/biromantic-alternate-2
            Self::Biromantic1 => ColorProfile::from_hex_colors(vec![
                "#8869A5", "#D8A7D8", "#FFFFFF", "#FDB18D", "#151638",
            ]),

            Self::Bisexual => ColorProfile::from_hex_colors(vec!["#D60270", "#9B4F96", "#0038A8"]),

            // sourced from https://www.flagcolorcodes.com/boyflux-alternate-2
            Self::Boyflux2 => ColorProfile::from_hex_colors(vec![
                "#E48AE4", "#9A81B4", "#55BFAB", "#FFFFFF", "#A8A8A8", "#81D5EF", "#69ABE5",
                "#5276D4",
            ])
            .and_then(|c| c.with_weights(vec![1, 1, 1, 1, 1, 5, 5, 5])),

            Self::Burger => ColorProfile::from_hex_colors(vec![
                "#F3A26A", "#498701", "#FD1C13", "#7D3829", "#F3A26A",
            ]),

            // sourced from https://www.flagcolorcodes.com/demiboy
            Self::Demiboy => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#9DD7EA", "#FFFFFF", "#9DD7EA", "#C4C4C4", "#7F7F7F",
            ]),

            // used colorpicker to source form https://www.deviantart.com/pride-flags/art/Demifae-870194777
            Self::Demifae => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#7F7F7F", "#C5C5C5", "#C5C5C5", "#97C3A4", "#C4DEAE", "#FFFFFF",
                "#FCA2C5", "#AB7EDF", "#C5C5C5", "#C5C5C5", "#7F7F7F", "#7F7F7F",
            ]),

            // sourced from https://www.flagcolorcodes.com/demifaun
            Self::Demifaun => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#7F7F7F", "#C6C6C6", "#C6C6C6", "#FCC688", "#FFF19C", "#FFFFFF",
                "#8DE0D5", "#9682EC", "#C6C6C6", "#C6C6C6", "#7F7F7F", "#7F7F7F",
            ]),

            // yellow sourced from https://lgbtqia.fandom.com/f/p/4400000000000041031
            // other colors sourced from demiboy and demigirl flags
            Self::Demigender => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#FBFF75", "#FFFFFF", "#FBFF75", "#C4C4C4", "#7F7F7F",
            ]),

            // sourced from https://www.flagcolorcodes.com/demigirl
            Self::Demigirl => ColorProfile::from_hex_colors(vec![
                "#7F7F7F", "#C4C4C4", "#FDADC8", "#FFFFFF", "#FDADC8", "#C4C4C4", "#7F7F7F",
            ]),

            Self::Femboy => ColorProfile::from_hex_colors(vec![
                "#d260a5", "#e4afcd", "#fefefe", "#57cef8", "#fefefe", "#e4afcd", "#d260a5",
            ]),

            // sourced from https://lgbtqia.wiki/wiki/Finsexual
            Self::Finsexual => ColorProfile::from_hex_colors(vec![
                "#B18EDF", "#D7B1E2", "#F7CDE9", "#F39FCE", "#EA7BB3",
            ]),

            // sampled from https://es.m.wikipedia.org/wiki/Archivo:Fraysexual_flag.jpg
            Self::Fraysexual => {
                ColorProfile::from_hex_colors(vec!["#226CB5", "#94E7DD", "#FFFFFF", "#636363"])
            },

            // sourced from https://www.flagcolorcodes.com/gay-men
            Self::GayMen => ColorProfile::from_hex_colors(vec![
                "#078D70", "#98E8C1", "#FFFFFF", "#7BADE2", "#3D1A78",
            ]),

            // sourced from https://www.flagcolorcodes.com/genderfae
            Self::Genderfae => ColorProfile::from_hex_colors(vec![
                "#97C3A5", "#C3DEAE", "#F9FACD", "#FFFFFF", "#FCA2C4", "#DB8AE4", "#A97EDD",
            ]),

            // sourced from https://www.flagcolorcodes.com/genderfaun
            Self::Genderfaun => ColorProfile::from_hex_colors(vec![
                "#FCD689", "#FFF09B", "#FAF9CD", "#FFFFFF", "#8EDED9", "#8CACDE", "#9782EC",
            ]),

            Self::Genderfluid => ColorProfile::from_hex_colors(vec![
                "#FE76A2", "#FFFFFF", "#BF12D7", "#000000", "#303CBE",
            ]),

            // sourced from https://www.deviantart.com/pride-flags/art/Genderflux-1-543925589
            Self::Genderflux => ColorProfile::from_hex_colors(vec![
                "f47694", "f2a2b9", "cecece", "7ce0f7", "3ecdf9", "fff48d",
            ]),

            Self::GenderNonconforming1 => ColorProfile::from_hex_colors(vec![
                "#50284d", "#96467b", "#5c96f7", "#ffe6f7", "#5c96f7", "#96467b", "#50284d",
            ])
            .and_then(|c| c.with_weights(vec![4, 1, 1, 1, 1, 1, 4])),

            Self::GenderNonconforming2 => ColorProfile::from_hex_colors(vec![
                "#50284d", "#96467b", "#5c96f7", "#ffe6f7", "#5c96f7", "#96467b", "#50284d",
            ]),

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Gendervoid => ColorProfile::from_hex_colors(vec![
                "#081149", "#4B484B", "#000000", "#4B484B", "#081149",
            ]),

            // sourced from https://commons.wikimedia.org/wiki/File:Girlflux_Pride_Flag.jpg
            Self::Girlflux => ColorProfile::from_hex_colors(vec![
                "f9e6d7", "f2526c", "bf0311", "e9c587", "bf0311", "f2526c", "f9e6d7",
            ]),

            // sourced from https://www.flagcolorcodes.com/greygender
            Self::Greygender => ColorProfile::from_hex_colors(vec![
                "#B3B3B3", "#B3B3B3", "#FFFFFF", "#062383", "#062383", "#FFFFFF", "#535353",
                "#535353",
            ]),

            // sourced from https://www.flagcolorcodes.com/greysexual
            Self::Greysexual => ColorProfile::from_hex_colors(vec![
                "#740194", "#AEB1AA", "#FFFFFF", "#AEB1AA", "#740194",
            ]),

            // sourced from https://lgbtqia.fandom.com/wiki/Gynesexual
            Self::Gynesexual => {
                ColorProfile::from_hex_colors(vec!["#F4A9B7", "#903F2B", "#5B953B"])
            },

            // sourced from https://www.flagcolorcodes.com/intergender
            Self::Intergender => ColorProfile::from_hex_colors(
                // todo: use weighted spacing
                vec!["#900DC2", "#900DC2", "#FFE54F", "#900DC2", "#900DC2"],
            ),

            Self::Lesbian => ColorProfile::from_hex_colors(vec![
                "#D62800", "#FF9B56", "#FFFFFF", "#D462A6", "#A40062",
            ]),

            // sourced from https://www.flagcolorcodes.com/neutrois
            Self::Neutrois => ColorProfile::from_hex_colors(vec!["#FFFFFF", "#1F9F00", "#000000"]),

            Self::Nonbinary => {
                ColorProfile::from_hex_colors(vec!["#FCF431", "#FCFCFC", "#9D59D2", "#282828"])
            },

            // used https://twitter.com/foxbrained/status/1667621855518236674/photo/1 as source and colorpicked
            Self::NonhumanUnity => {
                ColorProfile::from_hex_colors(vec!["#177B49", "#FFFFFF", "#593C90"])
            },

            Self::Omniromantic => ColorProfile::from_hex_colors(vec![
                "#FEC8E4", "#FDA1DB", "#89739A", "#ABA7FE", "#BFCEFF",
            ]),

            // sourced from https://www.flagcolorcodes.com/omnisexual
            Self::Omnisexual => ColorProfile::from_hex_colors(vec![
                "#FE9ACE", "#FF53BF", "#200044", "#6760FE", "#8EA6FF",
            ]),

            Self::Pangender => ColorProfile::from_hex_colors(vec![
                "#FFF798", "#FEDDCD", "#FFEBFB", "#FFFFFF", "#FFEBFB", "#FEDDCD", "#FFF798",
            ]),

            Self::Pansexual => ColorProfile::from_hex_colors(vec!["#FF1C8D", "#FFD700", "#1AB3FF"]),

            // used https://pluralpedia.org/w/Plurality#/media/File:Plural-Flag-1.jpg as source and colorpicked
            Self::Plural => ColorProfile::from_hex_colors(vec![
                "#2D0625", "#543475", "#7675C3", "#89C7B0", "#F3EDBD",
            ]),

            Self::Polysexual => {
                ColorProfile::from_hex_colors(vec!["#F714BA", "#01D66A", "#1594F6"])
            },

            Self::Queer => ColorProfile::from_hex_colors(vec!["#B57FDD", "#FFFFFF", "#49821E"]),

            Self::Rainbow => ColorProfile::from_hex_colors(vec![
                "#E50000", "#FF8D00", "#FFEE00", "#028121", "#004CFF", "#770088",
            ]),

            Self::Tomboy => ColorProfile::from_hex_colors(vec![
                "#2f3fb9", "#613a03", "#fefefe", "#f1a9b7", "#fefefe", "#613a03", "#2f3fb9",
            ]),

            // used colorpicker to source from https://www.deviantart.com/pride-flags/art/Trans-Woman-Transfeminine-1-543925985
            // linked from https://gender.fandom.com/wiki/Transfeminine
            Self::Transfeminine => ColorProfile::from_hex_colors(vec![
                "#73DEFF", "#FFE2EE", "#FFB5D6", "#FF8DC0", "#FFB5D6", "#FFE2EE", "#73DEFF",
            ]),

            Self::Transgender => ColorProfile::from_hex_colors(vec![
                "#55CDFD", "#F6AAB7", "#FFFFFF", "#F6AAB7", "#55CDFD",
            ]),

            // sourced from https://www.flagcolorcodes.com/transmasculine
            Self::Transmasculine => ColorProfile::from_hex_colors(vec![
                "#FF8ABD", "#CDF5FE", "#9AEBFF", "#74DFFF", "#9AEBFF", "#CDF5FE", "#FF8ABD",
            ]),

            // sourced from https://web.archive.org/web/20221002181913/https://unlabeledinfo.carrd.co/#flags
            Self::Unlabeled1 => {
                ColorProfile::from_hex_colors(vec!["#EAF8E4", "#FDFDFB", "#E1EFF7", "#F4E2C4"])
            },

            // sourced from https://web.archive.org/web/20221002181913/https://unlabeledinfo.carrd.co/#flags
            Self::Unlabeled2 => ColorProfile::from_hex_colors(vec![
                "#250548", "#FFFFFF", "#F7DCDA", "#EC9BEE", "#9541FA", "#7D2557",
            ]),

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Voidboy => ColorProfile::from_hex_colors(vec![
                "#0B130C", "#547655", "#66B969", "#547655", "#0B130C",
            ]),

            // sourced from: https://gender.fandom.com/wiki/Gendervoid
            Self::Voidgirl => ColorProfile::from_hex_colors(vec![
                "#180827", "#7A5A8B", "#E09BED", "#7A5A8B", "#180827",
            ]),
            // sourced from https://commons.wikimedia.org/wiki/File:Xenogender_pride_flag.svg
            Self::Xenogender => ColorProfile::from_hex_colors(vec![
                "#FF6692", "#FF9A98", "#FFB883", "#FBFFA8", "#85BCFF", "#9D85FF", "#A510FF",
            ]),
        })
        .expect("presets should not be invalid")
    }
}

impl ColorProfile {
    pub fn new(colors: Vec<RGB8>) -> Self {
        Self { colors }
    }

    pub fn from_hex_colors<S>(hex_colors: Vec<S>) -> Result<Self>
    where
        S: AsRef<str>,
    {
        let colors = hex_colors
            .into_iter()
            .map(RGB8::from_hex)
            .collect::<Result<_>>()
            .context("Failed to parse hex colors")?;
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
            Err(anyhow!(
                "`weights` should have the same number of elements as `colors`"
            ))?;
        }

        let mut weighted_colors = vec![];

        for (i, w) in weights.into_iter().enumerate() {
            weighted_colors.extend(iter::repeat(self.colors[i]).take(w as usize));
        }

        Ok(Self::new(weighted_colors))
    }

    /// Creates another color profile with only the unique colors.
    pub fn unique_colors(&self) -> Self {
        let unique_colors = self.colors.iter().collect::<IndexSet<_>>();
        let unique_colors = {
            let mut v = Vec::with_capacity(unique_colors.len());
            v.extend(unique_colors);
            v
        };
        Self::new(unique_colors)
    }
}
