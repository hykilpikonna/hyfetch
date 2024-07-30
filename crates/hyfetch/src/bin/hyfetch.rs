use std::borrow::Cow;
use std::cmp;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, IsTerminal as _, Read as _, Write as _};
use std::iter::zip;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::{Context as _, Result};
use deranged::RangedU8;
use enterpolation::bspline::BSpline;
use enterpolation::{Curve as _, Generator as _};
use hyfetch::ascii::RawAsciiArt;
use hyfetch::cli_options::options;
use hyfetch::color_util::{
    clear_screen, color, printc, ContrastGrayscale as _, ForegroundBackground, Lightness,
    NeofetchAsciiIndexedColor, PresetIndexedColor, Theme as _, ToAnsiString as _,
};
use hyfetch::models::Config;
#[cfg(feature = "macchina")]
use hyfetch::neofetch_util::macchina_path;
use hyfetch::neofetch_util::{
    self, fastfetch_path, get_distro_ascii, literal_input, ColorAlignment, NEOFETCH_COLORS_AC,
    NEOFETCH_COLOR_PATTERNS, TEST_ASCII,
};
use hyfetch::presets::{AssignLightness, Preset};
use hyfetch::pride_month;
use hyfetch::types::{AnsiMode, Backend, TerminalTheme};
use hyfetch::utils::{get_cache_path, input};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools as _;
use palette::{LinSrgb, Srgb};
use serde::Serialize as _;
use serde_json::ser::PrettyFormatter;
use strum::{EnumCount as _, VariantArray, VariantNames};
use terminal_colorsaurus::{background_color, QueryOptions};
use terminal_size::{terminal_size, Height, Width};
use time::{Month, OffsetDateTime};
use tracing::debug;

fn main() -> Result<()> {
    #[cfg(windows)]
    if let Err(err) = enable_ansi_support::enable_ansi_support() {
        debug!(%err, "could not enable ANSI escape code support");
    }

    let options = options().run();

    let debug_mode = options.debug;

    init_tracing_subsriber(debug_mode).context("failed to init tracing subscriber")?;

    debug!(?options, "CLI options");

    // Use a custom distro
    let distro = options.distro.as_ref();

    let backend = options.backend.unwrap_or(Backend::Neofetch);

    if options.test_print {
        let asc = get_distro_ascii(distro, backend).context("failed to get distro ascii")?;
        writeln!(io::stdout(), "{asc}", asc = asc.asc)
            .context("failed to write ascii to stdout")?;
        return Ok(());
    }

    let config = if options.config {
        create_config(&options.config_file, distro, backend, debug_mode)
            .context("failed to create config")?
    } else if let Some(config) =
        load_config(&options.config_file).context("failed to load config")?
    {
        config
    } else {
        create_config(&options.config_file, distro, backend, debug_mode)
            .context("failed to create config")?
    };

    let color_mode = options.mode.unwrap_or(config.mode);
    let theme = config.light_dark;

    // Check if it's June (pride month)
    let now =
        OffsetDateTime::now_local().context("failed to get current datetime in local timezone")?;
    let cache_path = get_cache_path().context("failed to get cache path")?;
    let june_path = cache_path.join(format!("animation-displayed-{year}", year = now.year()));
    let show_pride_month = options.june
        || now.month() == Month::June && !june_path.is_file() && io::stdout().is_terminal();

    if show_pride_month && !config.pride_month_disable {
        pride_month::start_animation(color_mode).context("failed to draw pride month animation")?;
        writeln!(
            io::stdout(),
            "\nHappy pride month!\n(You can always view the animation again with `hyfetch \
             --june`)\n"
        )
        .context("failed to write message to stdout")?;

        if !june_path.is_file() {
            fs::create_dir_all(&cache_path)
                .with_context(|| format!("failed to create cache dir {cache_path:?}"))?;
            File::create(&june_path)
                .with_context(|| format!("failed to create file {june_path:?}"))?;
        }
    }

    // Use a custom distro
    let distro = options.distro.as_ref().or(config.distro.as_ref());

    let backend = options.backend.unwrap_or(config.backend);
    let args = options.args.as_ref().or(config.args.as_ref());

    // Get preset
    let preset = options.preset.unwrap_or(config.preset);
    let color_profile = preset.color_profile();
    debug!(?color_profile, "color profile");

    // Lighten
    let color_profile = if let Some(scale) = options.scale {
        color_profile.lighten(scale)
    } else if let Some(lightness) = options.lightness {
        color_profile.with_lightness(AssignLightness::Replace(lightness))
    } else {
        color_profile.with_lightness_adaptive(config.lightness(), theme)
    };
    debug!(?color_profile, "lightened color profile");

    let asc = if let Some(path) = options.ascii_file {
        RawAsciiArt {
            asc: fs::read_to_string(&path)
                .with_context(|| format!("failed to read ascii from {path:?}"))?,
            fg: Vec::new(),
        }
    } else {
        get_distro_ascii(distro, backend).context("failed to get distro ascii")?
    };
    let asc = asc.to_normalized().context("failed to normalize ascii")?;
    let color_align = config.color_align;
    let asc = asc
        .to_recolored(&color_align, &color_profile, color_mode, theme)
        .context("failed to recolor ascii")?;
    neofetch_util::run(asc, backend, args)?;

    if options.ask_exit {
        input(Some("Press enter to exit...")).context("failed to read input")?;
    }

    Ok(())
}

/// Loads config from file.
///
/// Returns `None` if the config file does not exist.
#[tracing::instrument(level = "debug")]
fn load_config(path: &PathBuf) -> Result<Option<Config>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(None);
        },
        Err(err) => {
            return Err(err).with_context(|| format!("failed to open file {path:?} for reading"));
        },
    };

    let mut buf = String::new();

    file.read_to_string(&mut buf)
        .with_context(|| format!("failed to read from file {path:?}"))?;

    let deserializer = &mut serde_json::Deserializer::from_str(&buf);
    let config: Config = serde_path_to_error::deserialize(deserializer)
        .with_context(|| format!("failed to parse config from file {path:?}"))?;

    debug!(?config, "loaded config");

    Ok(Some(config))
}

/// Creates config interactively.
///
/// The config is automatically stored to file.
#[tracing::instrument(level = "debug")]
fn create_config(
    path: &PathBuf,
    distro: Option<&String>,
    backend: Backend,
    debug_mode: bool,
) -> Result<Config> {
    // Detect terminal environment (doesn't work for all terminal emulators,
    // especially on Windows)
    let det_bg = if io::stdout().is_terminal() {
        match background_color(QueryOptions::default()) {
            Ok(bg) => Some(Srgb::<u16>::new(bg.r, bg.g, bg.b).into_format::<u8>()),
            Err(terminal_colorsaurus::Error::UnsupportedTerminal) => None,
            Err(err) => {
                return Err(err).context("failed to get terminal background color");
            },
        }
    } else {
        None
    };
    debug!(?det_bg, "detected background color");
    let det_ansi = supports_color::on(supports_color::Stream::Stdout).map(|color_level| {
        if color_level.has_16m {
            AnsiMode::Rgb
        } else if color_level.has_256 {
            AnsiMode::Ansi256
        } else if color_level.has_basic {
            unimplemented!(
                "{mode} color mode not supported",
                mode = AnsiMode::Ansi16.as_ref()
            );
        } else {
            unreachable!();
        }
    });
    debug!(?det_ansi, "detected color mode");

    let asc = get_distro_ascii(distro, backend).context("failed to get distro ascii")?;
    let asc = asc.to_normalized().context("failed to normalize ascii")?;
    let theme = det_bg.map(|bg| bg.theme()).unwrap_or(TerminalTheme::Light);
    let color_mode = det_ansi.unwrap_or(AnsiMode::Ansi256);
    let mut title = format!(
        "Welcome to {logo} Let's set up some colors first.",
        logo = color(
            match theme {
                TerminalTheme::Light => "&l&bhyfetch&~&L",
                TerminalTheme::Dark => "&l&bhy&ffetch&~&L",
            },
            color_mode,
        )
        .expect("logo should not contain invalid color codes")
    );
    clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;

    let mut option_counter = NonZeroU8::new(1).unwrap();

    fn update_title(title: &mut String, option_counter: &mut NonZeroU8, k: &str, v: &str) {
        let k: Cow<str> = if k.ends_with(':') {
            k.into()
        } else {
            format!("{k}:").into()
        };
        write!(title, "\n&e{option_counter}. {k:<30} &~{v}").unwrap();
        *option_counter = option_counter
            .checked_add(1)
            .expect("`option_counter` should not overflow `u8`");
    }

    fn print_title_prompt(
        option_counter: NonZeroU8,
        prompt: &str,
        color_mode: AnsiMode,
    ) -> Result<()> {
        printc(format!("&a{option_counter}. {prompt}"), color_mode)
            .context("failed to print prompt")
    }

    //////////////////////////////
    // 0. Check term size

    {
        let (Width(term_w), Height(term_h)) =
            terminal_size().context("failed to get terminal size")?;
        let (term_w_min, term_h_min) = (
            u16::from(asc.w)
                .checked_mul(2)
                .unwrap()
                .checked_add(4)
                .unwrap(),
            30,
        );
        if term_w < term_w_min || term_h < term_h_min {
            printc(
                format!(
                    "&cWarning: Your terminal is too small ({term_w} * {term_h}).\nPlease resize \
                     it to at least ({term_w_min} * {term_h_min}) for better experience."
                ),
                color_mode,
            )
            .context("failed to print message")?;
            input(Some("Press enter to continue...")).context("failed to read input")?;
        }
    }

    //////////////////////////////
    // 1. Select color mode

    let default_color_profile = Preset::Rainbow.color_profile();

    let select_color_mode = || -> Result<(AnsiMode, &str)> {
        if det_ansi == Some(AnsiMode::Rgb) {
            return Ok((AnsiMode::Rgb, "Detected color mode"));
        }

        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;

        let (Width(term_w), _) = terminal_size().context("failed to get terminal size")?;

        let spline = BSpline::builder()
            .clamped()
            .elements(
                default_color_profile
                    .unique_colors()
                    .colors
                    .iter()
                    .map(|rgb_u8_color| rgb_u8_color.into_linear())
                    .collect::<Vec<_>>(),
            )
            .equidistant::<f32>()
            .degree(1)
            .normalized()
            .constant::<2>()
            .build()
            .expect("building spline should not fail");
        let [dmin, dmax] = spline.domain();
        let gradient: Vec<LinSrgb> = (0..term_w)
            .map(|i| spline.gen(remap(i as f32, 0.0, term_w as f32, dmin, dmax)))
            .collect();

        /// Maps `t` in range `[a, b)` to range `[c, d)`.
        fn remap(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
            (t - a) * ((d - c) / (b - a)) + c
        }

        {
            let label = format!(
                "{label:^term_w$}",
                label = "8bit Color Testing",
                term_w = usize::from(term_w)
            );
            let line = zip(gradient.iter(), label.chars()).fold(
                String::new(),
                |mut s, (&rgb_f32_color, t)| {
                    let rgb_u8_color = Srgb::<u8>::from_linear(rgb_f32_color);
                    let back = rgb_u8_color
                        .to_ansi_string(AnsiMode::Ansi256, ForegroundBackground::Background);
                    let fore = rgb_u8_color
                        .contrast_grayscale()
                        .to_ansi_string(AnsiMode::Ansi256, ForegroundBackground::Foreground);
                    write!(s, "{back}{fore}{t}").unwrap();
                    s
                },
            );
            printc(line, AnsiMode::Ansi256).context("failed to print 8-bit color test line")?;
        }
        {
            let label = format!(
                "{label:^term_w$}",
                label = "RGB Color Testing",
                term_w = usize::from(term_w)
            );
            let line = zip(gradient.iter(), label.chars()).fold(
                String::new(),
                |mut s, (&rgb_f32_color, t)| {
                    let rgb_u8_color = Srgb::<u8>::from_linear(rgb_f32_color);
                    let back = rgb_u8_color
                        .to_ansi_string(AnsiMode::Rgb, ForegroundBackground::Background);
                    let fore = rgb_u8_color
                        .contrast_grayscale()
                        .to_ansi_string(AnsiMode::Ansi256, ForegroundBackground::Foreground);
                    write!(s, "{back}{fore}{t}").unwrap();
                    s
                },
            );
            printc(line, AnsiMode::Rgb).context("failed to print RGB color test line")?;
        }

        writeln!(io::stdout()).context("failed to write to stdout")?;
        print_title_prompt(
            option_counter,
            "Which &bcolor system &ado you want to use?",
            color_mode,
        )
        .context("failed to print title prompt")?;
        writeln!(
            io::stdout(),
            "(If you can't see colors under \"RGB Color Testing\", please choose 8bit)\n"
        )
        .context("failed to write message to stdout")?;

        let choice = literal_input(
            "Your choice?",
            AnsiMode::VARIANTS,
            AnsiMode::Rgb.as_ref(),
            true,
            color_mode,
        )
        .context("failed to ask for choice input")?;
        Ok((
            choice.parse().expect("selected color mode should be valid"),
            "Selected color mode",
        ))
    };

    let color_mode = {
        let (color_mode, ttl) = select_color_mode().context("failed to select color mode")?;
        debug!(?color_mode, "selected color mode");
        update_title(&mut title, &mut option_counter, ttl, color_mode.as_ref());
        color_mode
    };

    //////////////////////////////
    // 2. Select theme (light/dark mode)

    let select_theme = || -> Result<(TerminalTheme, &str)> {
        if let Some(det_bg) = det_bg {
            return Ok((det_bg.theme(), "Detected background color"));
        }

        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;

        print_title_prompt(
            option_counter,
            "Is your terminal in &blight mode&~ or &4dark mode&~?",
            color_mode,
        )
        .context("failed to print title prompt")?;
        let choice = literal_input(
            "",
            TerminalTheme::VARIANTS,
            TerminalTheme::Dark.as_ref(),
            true,
            color_mode,
        )
        .context("failed to ask for choice input")?;
        Ok((
            choice.parse().expect("selected theme should be valid"),
            "Selected background color",
        ))
    };

    let theme = {
        let (theme, ttl) = select_theme().context("failed to select theme")?;
        debug!(?theme, "selected theme");
        update_title(&mut title, &mut option_counter, ttl, theme.as_ref());
        theme
    };

    //////////////////////////////
    // 3. Choose preset

    // Create flag lines
    let mut flags = Vec::with_capacity(Preset::COUNT);
    let spacing = {
        let spacing = <Preset as VariantNames>::VARIANTS
            .iter()
            .map(|name| name.chars().count())
            .max()
            .expect("preset name iterator should not be empty");
        let spacing: u8 = spacing.try_into().expect("`spacing` should fit in `u8`");
        cmp::max(spacing, 20)
    };
    for preset in <Preset as VariantArray>::VARIANTS {
        let color_profile = preset.color_profile();
        let flag = color_profile
            .color_text(
                " ".repeat(usize::from(spacing)),
                color_mode,
                ForegroundBackground::Background,
                false,
            )
            .with_context(|| format!("failed to color flag using preset: {preset:?}"))?;
        let name = format!(
            "{name:^spacing$}",
            name = preset.as_ref(),
            spacing = usize::from(spacing)
        );
        flags.push([name, flag.clone(), flag.clone(), flag]);
    }

    // Calculate flags per row
    let (flags_per_row, rows_per_page) = {
        let (Width(term_w), Height(term_h)) =
            terminal_size().context("failed to get terminal size")?;
        let flags_per_row = term_w.div_euclid(u16::from(spacing).checked_add(2).unwrap());
        let flags_per_row: u8 = flags_per_row
            .try_into()
            .expect("`flags_per_row` should fit in `u8`");
        let rows_per_page = cmp::max(1, term_h.saturating_sub(13).div_euclid(5));
        let rows_per_page: u8 = rows_per_page
            .try_into()
            .expect("`rows_per_page` should fit in `u8`");
        (flags_per_row, rows_per_page)
    };
    let num_pages =
        u16::from(u8::try_from(Preset::COUNT).expect("`Preset::COUNT` should fit in `u8`"))
            .div_ceil(
                u16::from(flags_per_row)
                    .checked_mul(u16::from(rows_per_page))
                    .unwrap(),
            );
    let num_pages: u8 = num_pages
        .try_into()
        .expect("`num_pages` should fit in `u8`");

    // Create pages
    let mut pages = Vec::with_capacity(usize::from(num_pages));
    for flags in flags.chunks(usize::from(
        u16::from(flags_per_row)
            .checked_mul(u16::from(rows_per_page))
            .unwrap(),
    )) {
        let mut page = Vec::with_capacity(usize::from(rows_per_page));
        for flags in flags.chunks(usize::from(flags_per_row)) {
            page.push(flags);
        }
        pages.push(page);
    }

    let print_flag_page = |page, page_num: u8| -> Result<()> {
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
        print_title_prompt(option_counter, "Let's choose a flag!", color_mode)
            .context("failed to print title prompt")?;
        writeln!(
            io::stdout(),
            "Available flag presets:\nPage: {page_num} of {num_pages}\n",
            page_num = page_num.checked_add(1).unwrap()
        )
        .context("failed to write header to stdout")?;
        for &row in page {
            print_flag_row(row, color_mode).context("failed to print flag row")?;
        }
        writeln!(io::stdout()).context("failed to write to stdout")?;
        Ok(())
    };

    fn print_flag_row(row: &[[String; 4]], color_mode: AnsiMode) -> Result<()> {
        for i in 0..4 {
            let mut line = Vec::new();
            for flag in row {
                line.push(&*flag[i]);
            }
            printc(line.join("  "), color_mode).context("failed to print line")?;
        }
        writeln!(io::stdout()).context("failed to write to stdout")?;
        Ok(())
    }

    let default_lightness = Config::default_lightness(theme);
    let preset_default_colored = default_color_profile
        .with_lightness_adaptive(default_lightness, theme)
        .color_text(
            "preset",
            color_mode,
            ForegroundBackground::Foreground,
            false,
        )
        .expect("coloring text with default preset should not fail");

    let preset: Preset;
    let color_profile;

    let mut page: u8 = 0;
    loop {
        print_flag_page(&pages[usize::from(page)], page).context("failed to print flag page")?;

        let mut opts: Vec<&str> = <Preset as VariantNames>::VARIANTS.into();
        if page < num_pages.checked_sub(1).unwrap() {
            opts.push("next");
        }
        if page > 0 {
            opts.push("prev");
        }
        writeln!(
            io::stdout(),
            "Enter 'next' to go to the next page and 'prev' to go to the previous page."
        )
        .context("failed to write message to stdout")?;
        let selection = literal_input(
            format!(
                "Which {preset} do you want to use? ",
                preset = preset_default_colored
            ),
            &opts[..],
            Preset::Rainbow.as_ref(),
            false,
            color_mode,
        )
        .context("failed to ask for choice input")
        .context("failed to select preset")?;
        if selection == "next" {
            page = page.checked_add(1).unwrap();
        } else if selection == "prev" {
            page = page.checked_sub(1).unwrap();
        } else {
            preset = selection.parse().expect("selected preset should be valid");
            debug!(?preset, "selected preset");
            color_profile = preset.color_profile();
            update_title(
                &mut title,
                &mut option_counter,
                "Selected flag",
                &color_profile
                    .with_lightness_adaptive(default_lightness, theme)
                    .color_text(
                        preset.as_ref(),
                        color_mode,
                        ForegroundBackground::Foreground,
                        false,
                    )
                    .expect("coloring text with selected preset should not fail"),
            );
            break;
        }
    }

    //////////////////////////////
    // 4. Dim/lighten colors

    let test_ascii = {
        let asc = &TEST_ASCII[1..TEST_ASCII.len().checked_sub(1).unwrap()];
        let asc = RawAsciiArt {
            asc: asc.to_owned(),
            fg: Vec::new(),
        };
        asc.to_normalized()
            .expect("normalizing test ascii should not fail")
    };

    let select_lightness = || -> Result<Lightness> {
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
        print_title_prompt(
            option_counter,
            "Let's adjust the color brightness!",
            color_mode,
        )
        .context("failed to print title prompt")?;
        writeln!(
            io::stdout(),
            "The colors might be a little bit too {bright_dark} for {light_dark} mode.\n",
            bright_dark = match theme {
                TerminalTheme::Light => "bright",
                TerminalTheme::Dark => "dark",
            },
            light_dark = theme.as_ref()
        )
        .context("failed to write message to stdout")?;

        let color_align = ColorAlignment::Horizontal;

        // Print cats
        {
            let (Width(term_w), _) = terminal_size().context("failed to get terminal size")?;
            let num_cols = cmp::max(
                1,
                term_w.div_euclid(u16::from(test_ascii.w).checked_add(2).unwrap()),
            );
            let num_cols: u8 = num_cols.try_into().expect("`num_cols` should fit in `u8`");
            const MIN: f32 = 0.15;
            const MAX: f32 = 0.85;
            let ratios =
                (0..num_cols)
                    .map(|col| col as f32 / num_cols as f32)
                    .map(|r| match theme {
                        TerminalTheme::Light => r * (MAX - MIN) / 2.0 + MIN,
                        TerminalTheme::Dark => (r * (MAX - MIN) + (MAX + MIN)) / 2.0,
                    });
            let row: Vec<Vec<String>> = ratios
                .map(|r| {
                    let mut asc = test_ascii.clone();
                    asc.lines = asc
                        .lines
                        .join("\n")
                        .replace(
                            "{txt}",
                            &format!(
                                "{lightness:^5}",
                                lightness = format!("{lightness:.0}%", lightness = r * 100.0)
                            ),
                        )
                        .lines()
                        .map(ToOwned::to_owned)
                        .collect();
                    let asc = asc
                        .to_recolored(
                            &color_align,
                            &color_profile.with_lightness_adaptive(
                                Lightness::new(r)
                                    .expect("generated lightness should not be invalid"),
                                theme,
                            ),
                            color_mode,
                            theme,
                        )
                        .expect("recoloring test ascii should not fail");
                    asc.lines
                })
                .collect();
            for i in 0..usize::from(test_ascii.h) {
                let mut line = Vec::new();
                for lines in &row {
                    line.push(&*lines[i]);
                }
                printc(line.join("  "), color_mode).context("failed to print test ascii line")?;
            }
        }

        loop {
            writeln!(
                io::stdout(),
                "\nWhich brightness level looks the best? (Default: {default:.0}% for \
                 {light_dark} mode)",
                default = f32::from(default_lightness) * 100.0,
                light_dark = theme.as_ref()
            )
            .context("failed to write prompt to stdout")?;
            let lightness = input(Some("> "))
                .context("failed to read input")?
                .trim()
                .to_lowercase();

            match parse_lightness(lightness, default_lightness) {
                Ok(lightness) => {
                    return Ok(lightness);
                },
                Err(err) => {
                    debug!(%err, "could not parse lightness");
                    printc(
                        "&cUnable to parse lightness value, please enter a lightness value such \
                         as 45%, .45, or 45",
                        color_mode,
                    )
                    .context("failed to print message")?;
                },
            }
        }
    };

    fn parse_lightness(lightness: String, default: Lightness) -> Result<Lightness> {
        if lightness.is_empty() || ["unset", "none"].contains(&&*lightness) {
            return Ok(default);
        }

        let lightness = if let Some(lightness) = lightness.strip_suffix('%') {
            let lightness: RangedU8<0, 100> = lightness.parse()?;
            lightness.get() as f32 / 100.0
        } else {
            match lightness.parse::<RangedU8<0, 100>>() {
                Ok(lightness) => lightness.get() as f32 / 100.0,
                Err(_) => lightness.parse::<f32>()?,
            }
        };

        Ok(Lightness::new(lightness)?)
    }

    let lightness = select_lightness().context("failed to select lightness")?;
    debug!(?lightness, "selected lightness");
    let color_profile = color_profile.with_lightness_adaptive(lightness, theme);
    update_title(
        &mut title,
        &mut option_counter,
        "Selected brightness",
        &format!("{lightness:.2}", lightness = f32::from(lightness)),
    );

    //////////////////////////////
    // 5. Color arrangement

    let color_align: ColorAlignment;

    // Calculate amount of row/column that can be displayed on screen
    let (ascii_per_row, ascii_rows) = {
        let (Width(term_w), Height(term_h)) =
            terminal_size().context("failed to get terminal size")?;
        let ascii_per_row = cmp::max(
            1,
            term_w.div_euclid(u16::from(asc.w).checked_add(2).unwrap()),
        );
        let ascii_per_row: u8 = ascii_per_row
            .try_into()
            .expect("`ascii_per_row` should fit in `u8`");
        let ascii_rows = cmp::max(
            1,
            term_h
                .saturating_sub(8)
                .div_euclid(u16::from(asc.h).checked_add(1).unwrap()),
        );
        let ascii_rows: u8 = ascii_rows
            .try_into()
            .expect("`ascii_rows` should fit in `u8`");
        (ascii_per_row, ascii_rows)
    };

    // Displays horizontal and vertical arrangements in the first iteration, but
    // hide them in later iterations
    let hv_arrangements = [
        ("Horizontal", ColorAlignment::Horizontal),
        ("Vertical", ColorAlignment::Vertical),
    ];
    let mut arrangements: IndexMap<Cow<str>, ColorAlignment> =
        hv_arrangements.map(|(k, ca)| (k.into(), ca)).into();

    let slots: IndexSet<NeofetchAsciiIndexedColor> = {
        let asc = asc.lines.join("\n");
        let ac =
            NEOFETCH_COLORS_AC.get_or_init(|| AhoCorasick::new(NEOFETCH_COLOR_PATTERNS).unwrap());
        ac.find_iter(&asc)
            .map(|m| {
                let ai_start = m.start().checked_add(3).unwrap();
                let ai_end = m.end().checked_sub(1).unwrap();
                asc[ai_start..ai_end]
                    .parse()
                    .expect("neofetch ascii color index should not be invalid")
            })
            .collect()
    };

    // Loop for random rolling
    let mut rng = fastrand::Rng::new();
    loop {
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;

        // Random color schemes
        let mut preset_indices: Vec<PresetIndexedColor> =
            (0..color_profile.unique_colors().colors.len())
                .map(|pi| u8::try_from(pi).expect("`pi` should fit in `u8`").into())
                .collect();
        while preset_indices.len() < slots.len() {
            preset_indices.extend_from_within(0..);
        }
        let preset_index_permutations: IndexSet<Vec<PresetIndexedColor>> = preset_indices
            .into_iter()
            .permutations(slots.len())
            .collect();
        let random_count = u16::from(ascii_per_row)
            .checked_mul(u16::from(ascii_rows))
            .unwrap()
            .saturating_sub(u8::try_from(arrangements.len()).unwrap().into());
        let random_count: u8 = random_count
            .try_into()
            .expect("`random_count` should fit in `u8`");
        let choices: IndexSet<Vec<PresetIndexedColor>> =
            if usize::from(random_count) > preset_index_permutations.len() {
                preset_index_permutations
            } else {
                rng.choose_multiple(
                    preset_index_permutations.into_iter(),
                    usize::from(random_count),
                )
                .into_iter()
                .collect()
            };
        let choices: Vec<IndexMap<NeofetchAsciiIndexedColor, PresetIndexedColor>> = choices
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .enumerate()
                    .map(|(ai, pi)| (slots[ai], pi))
                    .collect()
            })
            .collect();
        arrangements.extend(choices.into_iter().enumerate().map(|(i, colors)| {
            (format!("random{i}").into(), ColorAlignment::Custom {
                colors,
            })
        }));
        let asciis: Vec<Vec<String>> = arrangements
            .iter()
            .map(|(k, ca)| {
                let mut v: Vec<String> = asc
                    .to_recolored(ca, &color_profile, color_mode, theme)
                    .context("failed to recolor ascii")?
                    .lines;
                v.push(format!("{k:^asc_width$}", asc_width = usize::from(asc.w)));
                Ok(v)
            })
            .collect::<Result<_>>()?;

        for row in &asciis.into_iter().chunks(usize::from(ascii_per_row)) {
            let row: Vec<Vec<String>> = row.collect();

            // Print by row
            for i in 0..usize::from(asc.h).checked_add(1).unwrap() {
                let mut line = Vec::new();
                for lines in &row {
                    line.push(&*lines[i]);
                }
                printc(line.join("  "), color_mode).context("failed to print ascii line")?;
            }
            writeln!(io::stdout()).context("failed to write to stdout")?;
        }

        print_title_prompt(
            option_counter,
            "Let's choose a color arrangement!",
            color_mode,
        )
        .context("failed to print title prompt")?;
        writeln!(
            io::stdout(),
            "You can choose standard horizontal or vertical alignment, or use one of the random \
             color schemes.\nYou can type \"roll\" to randomize again.\n"
        )
        .context("failed to write message to stdout")?;
        let mut opts: Vec<Cow<str>> = ["horizontal", "vertical", "roll"].map(Into::into).into();
        opts.extend((0..random_count).map(|i| format!("random{i}").into()));
        let choice = literal_input("Your choice?", &opts[..], "horizontal", true, color_mode)
            .context("failed to ask for choice input")
            .context("failed to select color alignment")?;

        if choice == "roll" {
            arrangements.clear();
            continue;
        }

        // Save choice
        color_align = arrangements
            .into_iter()
            .find_map(|(k, ca)| {
                if k.to_lowercase() == choice {
                    Some(ca)
                } else {
                    None
                }
            })
            .expect("selected color alignment should be valid");
        debug!(?color_align, "selected color alignment");
        break;
    }

    update_title(
        &mut title,
        &mut option_counter,
        "Selected color alignment",
        color_align.as_ref(),
    );

    //////////////////////////////
    // 6. Select *fetch backend

    let select_backend = || -> Result<Backend> {
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
        print_title_prompt(option_counter, "Select a *fetch backend", color_mode)
            .context("failed to print title prompt")?;

        // Check if fastfetch is installed
        let fastfetch_path = fastfetch_path().context("failed to get fastfetch path")?;

        // Check if macchina is installed
        #[cfg(feature = "macchina")]
        let macchina_path = macchina_path().context("failed to get macchina path")?;

        printc(
            "- &bneofetch&r: Written in bash, &nbest compatibility&r on Unix systems",
            color_mode,
        )
        .context("failed to print message")?;
        printc(
            format!(
                "- &bfastfetch&r: Written in C, &nbest performance&r {installed_not_installed}",
                installed_not_installed = fastfetch_path
                    .map(|path| format!("&a(Installed at {path})", path = path.display()))
                    .unwrap_or_else(|| "&c(Not installed)".to_owned())
            ),
            color_mode,
        )
        .context("failed to print message")?;
        #[cfg(feature = "macchina")]
        printc(
            format!(
                "- &bmacchina&r: Written in Rust, &nbest performance&r {installed_not_installed}\n",
                installed_not_installed = macchina_path
                    .map(|path| format!("&a(Installed at {path})", path = path.display()))
                    .unwrap_or_else(|| "&c(Not installed)".to_owned())
            ),
            color_mode,
        )
        .context("failed to print message")?;

        let choice = literal_input(
            "Your choice?",
            Backend::VARIANTS,
            backend.as_ref(),
            true,
            color_mode,
        )
        .context("failed to ask for choice input")?;
        Ok(choice.parse().expect("selected backend should be valid"))
    };

    let backend = select_backend().context("failed to select backend")?;
    update_title(
        &mut title,
        &mut option_counter,
        "Selected backend",
        backend.as_ref(),
    );

    // Create config
    clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
    let config = Config {
        preset,
        mode: color_mode,
        light_dark: theme,
        lightness: Some(lightness),
        color_align,
        backend,
        args: None,
        distro: distro.cloned(),
        pride_month_disable: false,
    };
    debug!(?config, "created config");

    // Save config
    let save = literal_input("Save config?", &["y", "n"], "y", true, color_mode)
        .context("failed to ask for choice input")?;
    if save == "y" {
        match path.parent().context("invalid config file path")? {
            parent_path if parent_path != Path::new("") => {
                fs::create_dir_all(parent_path)
                    .with_context(|| format!("failed to create dir {parent_path:?}"))?;
            },
            _ => {
                // Nothing to do if it's a relative path with one component
            },
        }
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("failed to open file {path:?} for writing"))?;
        let mut serializer =
            serde_json::Serializer::with_formatter(file, PrettyFormatter::with_indent(b"    "));
        config
            .serialize(&mut serializer)
            .with_context(|| format!("failed to write config to file {path:?}"))?;
        debug!(?path, "saved config");
    }

    Ok(config)
}

fn init_tracing_subsriber(debug_mode: bool) -> Result<()> {
    use std::env;
    use std::str::FromStr as _;

    use tracing::Level;
    use tracing_subscriber::filter::{LevelFilter, Targets};
    use tracing_subscriber::fmt::Subscriber;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    let builder = Subscriber::builder();

    // Remove the default max level filter from the subscriber; it will be added to
    // the `Targets` filter instead if no filter is set in `RUST_LOG`.
    // Replacing the default `LevelFilter` with an `EnvFilter` would imply this,
    // but we can't replace the builder's filter with a `Targets` filter yet.
    let builder = builder.with_max_level(LevelFilter::TRACE);

    let subscriber = builder.finish();
    let subscriber = {
        let targets = match env::var("RUST_LOG") {
            Ok(var) => Targets::from_str(&var)
                .map_err(|e| {
                    eprintln!("Ignoring `RUST_LOG={var:?}`: {e}");
                })
                .unwrap_or_default(),
            Err(env::VarError::NotPresent) => {
                Targets::new().with_default(Subscriber::DEFAULT_MAX_LEVEL)
            },
            Err(e) => {
                eprintln!("Ignoring `RUST_LOG`: {e}");
                Targets::new().with_default(Subscriber::DEFAULT_MAX_LEVEL)
            },
        };
        let targets = if debug_mode {
            targets.with_target(env!("CARGO_CRATE_NAME"), Level::DEBUG)
        } else {
            targets
        };
        subscriber.with(targets)
    };

    subscriber
        .try_init()
        .context("failed to set the global default subscriber")
}
