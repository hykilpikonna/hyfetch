use std::borrow::Cow;
use std::fmt::Write as _;
use std::ops::Range;

use aho_corasick::AhoCorasick;
use anyhow::{Context as _, Result};
use indexmap::IndexMap;
use tracing::debug;
use unicode_segmentation::UnicodeSegmentation;

use crate::color_util::{
    color, ForegroundBackground, NeofetchAsciiIndexedColor, ToAnsiString as _,
};
use crate::neofetch_util::{
    ascii_size, ColorAlignment, NEOFETCH_COLORS_AC, NEOFETCH_COLOR_PATTERNS,
};
use crate::presets::ColorProfile;
use crate::types::{AnsiMode, TerminalTheme};

/// Raw ascii art before any processing.
#[derive(Clone, Debug)]
pub struct RawAsciiArt {
    pub asc: String,
    pub fg: Vec<NeofetchAsciiIndexedColor>,
}

/// Normalized ascii art where every line has the same width.
#[derive(Clone, Debug)]
pub struct NormalizedAsciiArt {
    pub lines: Vec<String>,
    pub w: u8,
    pub h: u8,
    pub fg: Vec<NeofetchAsciiIndexedColor>,
}

/// Recolored ascii art with all color codes replaced.
#[derive(Clone, Debug)]
pub struct RecoloredAsciiArt {
    pub lines: Vec<String>,
    pub w: u8,
    pub h: u8,
}

impl RawAsciiArt {
    /// Makes sure every line is the same width.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn to_normalized(&self) -> Result<NormalizedAsciiArt> {
        debug!("normalize ascii");

        let (w, h) = ascii_size(&self.asc).context("failed to get ascii size")?;

        let lines = self
            .asc
            .lines()
            .map(|line| {
                let (line_w, _) = ascii_size(line).unwrap();
                let pad = " ".repeat(usize::from(w.checked_sub(line_w).unwrap()));
                format!("{line}{pad}")
            })
            .collect();

        Ok(NormalizedAsciiArt {
            lines,
            w,
            h,
            fg: self.fg.clone(),
        })
    }
}

impl NormalizedAsciiArt {
    /// Uses a color alignment to recolor the ascii art.
    #[tracing::instrument(level = "debug", skip(self), fields(self.w = self.w, self.h = self.h))]
    pub fn to_recolored(
        &self,
        color_align: &ColorAlignment,
        color_profile: &ColorProfile,
        color_mode: AnsiMode,
        theme: TerminalTheme,
    ) -> Result<RecoloredAsciiArt> {
        debug!("recolor ascii");

        if self.lines.is_empty() {
            return Ok(RecoloredAsciiArt {
                lines: self.lines.clone(),
                w: 0,
                h: 0,
            });
        }

        let reset = color("&~&*", color_mode).expect("color reset should not be invalid");

        let lines = match (color_align, self) {
            (ColorAlignment::Horizontal, Self { fg, .. }) => {
                let Self { lines, .. } = self
                    .fill_starting()
                    .context("failed to fill in starting neofetch color codes")?;

                let ac = NEOFETCH_COLORS_AC
                    .get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());

                // Replace foreground colors
                let asc = {
                    let asc = lines.join("\n");
                    let mut replacements = NEOFETCH_COLOR_PATTERNS;
                    let fg_color = color(
                        match theme {
                            TerminalTheme::Light => "&0",
                            TerminalTheme::Dark => "&f",
                        },
                        color_mode,
                    )
                    .expect("foreground color should not be invalid");
                    for &fore in fg {
                        replacements[usize::from(u8::from(fore)).checked_sub(1).unwrap()] =
                            &fg_color;
                    }
                    ac.replace_all(&asc, &replacements)
                };
                let lines = asc.lines();

                // Add new colors
                let lines = {
                    let ColorProfile { colors } = color_profile
                        .with_length(self.h.try_into().expect("`h` should not be 0"))
                        .with_context(|| {
                            format!("failed to spread color profile to length {h}", h = self.h)
                        })?;
                    lines.enumerate().map(move |(i, line)| {
                        let bg_color =
                            colors[i].to_ansi_string(color_mode, ForegroundBackground::Foreground);
                        const N: usize = NEOFETCH_COLOR_PATTERNS.len();
                        let replacements = [&bg_color; N];
                        ac.replace_all(line, &replacements)
                    })
                };

                // Reset colors at end of each line to prevent color bleeding
                lines.map(|line| format!("{line}{reset}")).collect()
            },
            (ColorAlignment::Vertical, Self { fg, .. }) if !fg.is_empty() => {
                if self.w == 0 {
                    return Ok(RecoloredAsciiArt {
                        lines: self.lines.clone(),
                        w: 0,
                        h: self.h,
                    });
                }

                let Self { lines, .. } = self
                    .fill_starting()
                    .context("failed to fill in starting neofetch color codes")?;

                let color_profile = color_profile
                    .with_length(self.w.try_into().expect("`w` should not be 0"))
                    .with_context(|| {
                        format!("failed to spread color profile to length {w}", w = self.w)
                    })?;

                // Apply colors
                let ac = NEOFETCH_COLORS_AC
                    .get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());
                lines
                    .into_iter()
                    .map(|line| {
                        let line: &str = line.as_ref();

                        // `AhoCorasick` operates on bytes; we need to map that back to grapheme
                        // clusters (i.e. a character as seen on the terminal)
                        // See https://github.com/BurntSushi/aho-corasick/issues/72#issuecomment-821128859
                        let byte_idx_to_grapheme_idx: IndexMap<usize, usize> = {
                            let mut m: IndexMap<_, _> = line
                                .grapheme_indices(true)
                                .enumerate()
                                .map(|(gr_idx, (byte_idx, _))| (byte_idx, gr_idx))
                                .collect();
                            // Add an extra entry at the end, to support lookup using exclusive
                            // range end
                            m.insert(line.len(), m.len());
                            m
                        };

                        let mut matches = ac.find_iter(line).peekable();
                        let mut dst = String::new();
                        let mut offset: u8 = 0;
                        loop {
                            let current = matches.next();
                            let next = matches.peek();
                            let (neofetch_color_idx, span, done) = match (current, next) {
                                (Some(m), Some(m_next)) => {
                                    let ai_start = m.start().checked_add(3).unwrap();
                                    let ai_end = m.end().checked_sub(1).unwrap();
                                    let neofetch_color_idx: NeofetchAsciiIndexedColor = line
                                        [ai_start..ai_end]
                                        .parse()
                                        .expect("neofetch color index should be valid");
                                    if offset == 0 && m.start() > 0 {
                                        dst.push_str(&line[..m.start()]);
                                    }
                                    offset =
                                        offset.checked_add(u8::try_from(m.len()).unwrap()).unwrap();
                                    let mut span = m.span();
                                    span.start = m.end();
                                    span.end = m_next.start();
                                    (neofetch_color_idx, span, false)
                                },
                                (Some(m), None) => {
                                    // Last color code
                                    let ai_start = m.start().checked_add(3).unwrap();
                                    let ai_end = m.end().checked_sub(1).unwrap();
                                    let neofetch_color_idx: NeofetchAsciiIndexedColor = line
                                        [ai_start..ai_end]
                                        .parse()
                                        .expect("neofetch color index should be valid");
                                    if offset == 0 && m.start() > 0 {
                                        dst.push_str(&line[..m.start()]);
                                    }
                                    offset =
                                        offset.checked_add(u8::try_from(m.len()).unwrap()).unwrap();
                                    let mut span = m.span();
                                    span.start = m.end();
                                    span.end = line.len();
                                    (neofetch_color_idx, span, true)
                                },
                                (None, _) => {
                                    // No color code in the entire line
                                    unreachable!(
                                        "`fill_starting` ensured each line of ascii art starts \
                                         with neofetch color code"
                                    );
                                },
                            };

                            if span.is_empty() {
                                continue;
                            }

                            let txt = &line[span];

                            if fg.contains(&neofetch_color_idx) {
                                let fore = color(
                                    match theme {
                                        TerminalTheme::Light => "&0",
                                        TerminalTheme::Dark => "&f",
                                    },
                                    color_mode,
                                )
                                .expect("foreground color should not be invalid");
                                write!(dst, "{fore}{txt}{reset}").unwrap();
                            } else {
                                let mut c_range: Range<usize> = span.into();
                                c_range.start = byte_idx_to_grapheme_idx
                                    .get(&c_range.start)
                                    .unwrap()
                                    .checked_sub(usize::from(offset))
                                    .unwrap();
                                c_range.end = byte_idx_to_grapheme_idx
                                    .get(&c_range.end)
                                    .unwrap()
                                    .checked_sub(usize::from(offset))
                                    .unwrap();
                                dst.push_str(
                                    &ColorProfile::new(Vec::from(&color_profile.colors[c_range]))
                                        .color_text(
                                            txt,
                                            color_mode,
                                            ForegroundBackground::Foreground,
                                            false,
                                        )
                                        .context("failed to color text using color profile")?,
                                );
                            }

                            if done {
                                break;
                            }
                        }
                        Ok(dst)
                    })
                    .collect::<Result<_>>()?
            },
            (ColorAlignment::Vertical, Self { fg, .. }) if fg.is_empty() => {
                // Remove existing colors
                let asc = {
                    let asc = self.lines.join("\n");
                    let ac = NEOFETCH_COLORS_AC
                        .get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());
                    const N: usize = NEOFETCH_COLOR_PATTERNS.len();
                    const REPLACEMENTS: [&str; N] = [""; N];
                    ac.replace_all(&asc, &REPLACEMENTS)
                };
                let lines = asc.lines();

                // Add new colors
                lines
                    .map(|line| {
                        let line = color_profile
                            .color_text(line, color_mode, ForegroundBackground::Foreground, false)
                            .context("failed to color text using color profile")?;
                        Ok(line)
                    })
                    .collect::<Result<_>>()?
            },
            (
                ColorAlignment::Custom {
                    colors: custom_colors,
                },
                _,
            ) => {
                let Self { lines, .. } = self
                    .fill_starting()
                    .context("failed to fill in starting neofetch color codes")?;

                let ColorProfile { colors } = color_profile.unique_colors();

                // Apply colors
                let asc = {
                    let asc = lines.join("\n");
                    let ac = NEOFETCH_COLORS_AC
                        .get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());
                    const N: usize = NEOFETCH_COLOR_PATTERNS.len();
                    let mut replacements = vec![Cow::from(""); N];
                    for (&ai, &pi) in custom_colors {
                        let ai: u8 = ai.into();
                        let pi: u8 = pi.into();
                        replacements[usize::from(ai.checked_sub(1).unwrap())] = colors
                            [usize::from(pi)]
                        .to_ansi_string(color_mode, ForegroundBackground::Foreground)
                        .into();
                    }
                    ac.replace_all(&asc, &replacements)
                };
                let lines = asc.lines();

                // Reset colors at end of each line to prevent color bleeding
                lines.map(|line| format!("{line}{reset}")).collect()
            },
            _ => {
                unreachable!()
            },
        };

        Ok(RecoloredAsciiArt {
            lines,
            w: self.w,
            h: self.h,
        })
    }

    /// Fills the missing starting placeholders.
    ///
    /// e.g. `"${c1}...\n..."` -> `"${c1}...\n${c1}..."`
    fn fill_starting(&self) -> Result<Self> {
        let ac =
            NEOFETCH_COLORS_AC.get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());

        let mut last = None;
        let lines =
            self.lines
                .iter()
                .map(|line| {
                    let line: &str = line.as_ref();

                    let mut new = String::new();
                    let mut matches = ac.find_iter(line).peekable();

                    match matches.peek() {
                        Some(m)
                            if m.start() == 0
                                || line[0..m.start()].trim_end_matches(' ').is_empty() =>
                        {
                            // Line starts with neofetch color code
                            last = Some(&line[m.span()]);
                        },
                        Some(_) => {
                            new.push_str(last.context(
                                "failed to find neofetch color code from a previous line",
                            )?);
                        },
                        None => {
                            new.push_str(last.unwrap_or(NEOFETCH_COLOR_PATTERNS[0]));
                        },
                    }
                    new.push_str(line);

                    // Get the last placeholder for the next line
                    if let Some(m) = matches.last() {
                        last.context("non-space character seen before first color code")?;
                        last = Some(&line[m.span()]);
                    }

                    Ok(new)
                })
                .collect::<Result<_>>()?;

        Ok(Self {
            lines,
            fg: self.fg.clone(),
            ..*self
        })
    }
}
