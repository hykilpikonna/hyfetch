use std::borrow::Cow;
use std::cmp;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, IsTerminal as _, Read as _, Write};
use std::iter;
use std::iter::zip;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::{Context as _, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use deranged::RangedU8;
use enterpolation::bspline::BSpline;
use enterpolation::{Curve as _, Generator as _};
use hyfetch::ascii::RawAsciiArt;
use hyfetch::ascii::NormalizedAsciiArt;
use hyfetch::cli_options::options;
use hyfetch::color_util::{
    clear_screen, color, printc, ContrastGrayscale as _, ForegroundBackground, Lightness,
    NeofetchAsciiIndexedColor, PresetIndexedColor, Theme as _, ToAnsiString as _,
};
use hyfetch::distros::Distro;
use hyfetch::models::Config;
#[cfg(feature = "macchina")]
use hyfetch::neofetch_util::macchina_path;
use hyfetch::neofetch_util::{self, add_pkg_path, fastfetch_path, get_distro_ascii, get_distro_name, literal_input, ColorAlignment, NEOFETCH_COLORS_AC, NEOFETCH_COLOR_PATTERNS, TEST_ASCII};
use hyfetch::color_profile::{AssignLightness, ColorProfile};
use hyfetch::presets::{Preset};
use hyfetch::{pride_month, printc};
use hyfetch::types::{AnsiMode, Backend, TerminalTheme};
use hyfetch::utils::{get_cache_path, input};
use hyfetch::font_logo::get_font_logo;
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
    add_pkg_path().expect("failed to add pkg path");
    
    #[cfg(windows)]
    if let Err(err) = enable_ansi_support::enable_ansi_support() {
        debug!(%err, "could not enable ANSI escape code support");
    }

    let options = options().run();

    // Read ascii file first to avoid any issues with file descriptors being closed by other operations, and to allow CLI arguments to override config. (https://github.com/hykilpikonna/hyfetch/issues/475)
    let cli_ascii = if let Some(path) = &options.ascii_file {
        Some(RawAsciiArt {
            asc: fs::read_to_string(path).with_context(|| format!("failed to read ascii from {path:?}"))?,
            fg: Vec::new(),
        })
    } else {
        None
    };

    let debug_mode = options.debug;

    init_tracing_subsriber(debug_mode).context("failed to init tracing subscriber")?;

    debug!(?options, "CLI options");

    // Use a custom distro
    let distro = options.distro.as_ref();

    let backend = options.backend.unwrap_or_else(|| {
        if fastfetch_path().is_ok() { Backend::Fastfetch } else { Backend::Neofetch }
    });

    if options.test_print {
        println!("{asc}", asc = get_distro_ascii(distro, backend)?.asc);
        return Ok(());
    }

    if options.print_font_logo {
        println!("{}", get_font_logo(backend)?);
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
    let auto_detect_light_dark = options
        .auto_detect_light_dark
        .unwrap_or_else(|| config.auto_detect_light_dark.unwrap_or(false));
    let theme = if auto_detect_light_dark {
        let res = det_bg();
        res?.map(|bg| bg.theme())
            .unwrap_or(config.light_dark.unwrap_or_default())
    } else {
        config.light_dark.unwrap_or_default()
    };

    // Check if it's June (pride month)
    let now =
        OffsetDateTime::now_local().context("failed to get current datetime in local timezone")?;
    let cache_path = get_cache_path().context("failed to get cache path")?;
    let june_path = cache_path.join(format!("animation-displayed-{year}", year = now.year()));
    let show_pride_month = options.june
        || now.month() == Month::June && !june_path.is_file() && io::stdout().is_terminal();

    if show_pride_month && !config.pride_month_disable {
        pride_month::start_animation(color_mode).context("failed to draw pride month animation")?;
        println!("\nHappy pride month!\n(You can always view the animation again with `hyfetch --june`)\n");

        if !june_path.is_file() && !options.june {
            File::create(&june_path)
                .with_context(|| format!("failed to create file {june_path:?}"))?;
        }
    }

    // Use a custom distro
    let distro = options.distro.as_ref().or(config.distro.as_ref());

    let backend = options.backend.unwrap_or(config.backend);
    let args = options.args.as_ref().or(config.args.as_ref());

    fn parse_preset_string(preset_string: &str) -> Result<ColorProfile> {
        if preset_string.contains('#') {
            let colors: Vec<&str> = preset_string.split(',').map(|s| s.trim()).collect();
            for color in &colors {
                if !color.starts_with('#') ||
                    (color.len() != 4 && color.len() != 7) ||
                    !color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(anyhow::anyhow!("invalid hex color: {}", color));
                }
            }
            ColorProfile::from_hex_colors(colors)
                .context("failed to create color profile from hex")
        } else if preset_string == "random" {
            let mut rng = fastrand::Rng::new();
            let preset = *rng
                .choice(<Preset as VariantArray>::VARIANTS)
                .expect("preset iterator should not be empty");
            Ok(preset.color_profile())
        } else {
            use std::str::FromStr;
            let preset = Preset::from_str(preset_string)
                .with_context(|| {
                    format!(
                        "PRESET should be comma-separated hex colors or one of {{{presets}}}",
                        presets = <Preset as VariantNames>::VARIANTS
                            .iter()
                            .chain(iter::once(&"random"))
                            .join(",")
                    )
                })?;
            Ok(preset.color_profile())
        }
    }

    // Get preset
    let preset_string = options.preset.as_deref().unwrap_or(&config.preset);
    let color_profile = parse_preset_string(preset_string)?;
    debug!(?color_profile, "color profile");

    // Lighten
    let color_profile = if let Some(scale) = options.scale {
        color_profile.lighten(scale)
    } else if let Some(lightness) = options.lightness {
        color_profile.with_lightness(AssignLightness::Replace(lightness))
    } else {
        color_profile.with_lightness_adaptive(
            config
                .lightness
                .unwrap_or_else(|| Config::default_lightness(theme)),
            theme,
        )
    };
    debug!(?color_profile, "lightened color profile");

    let asc = if let Some(asc) = cli_ascii {
        asc
    } else if let Some(path_str) = config.custom_ascii_path {
        let path = PathBuf::from(path_str);
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

fn det_bg() -> Result<Option<Srgb<u8>>, terminal_colorsaurus::Error> {
    if !io::stdout().is_terminal() {
        return Ok(None);
    }

    background_color(QueryOptions::default())
        .map(|terminal_colorsaurus::Color { r, g, b , .. }| Some(Srgb::new(r, g, b).into_format()))
        .or_else(|err| {
            if matches!(err, terminal_colorsaurus::Error::UnsupportedTerminal(_)) {
                Ok(None)
            } else {
                Err(err)
            }
        })
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
    let det_bg = det_bg()?;
    debug!(?det_bg, "detected background color");
    let det_ansi = supports_color::on(supports_color::Stream::Stdout).map(|color_level| {
        #[allow(clippy::if_same_then_else)]
        if color_level.has_16m {
            AnsiMode::Rgb
        } else if color_level.has_256 {
            AnsiMode::Ansi256
        } else if color_level.has_basic {
            AnsiMode::Ansi256
        } else {
            unreachable!();
        }
    });
    debug!(?det_ansi, "detected color mode");

    // Try to get ascii from given backend first, if it fails, try neofetch backend
    let asc = get_distro_ascii(distro, backend).or_else(|_| get_distro_ascii(distro, Backend::Neofetch)).context("failed to get distro ascii from neofetch backend")?;
    let asc = asc.to_normalized().context("failed to normalize ascii")?;
    let theme = det_bg.map(|bg| bg.theme()).unwrap_or_default();
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

    fn print_title_prompt(option_counter: NonZeroU8, prompt: &str) {
        printc!("&a{option_counter}. {prompt}");
    }

    //////////////////////////////
    // 0. Check term size

    {
        let (Width(term_w), Height(term_h)) = terminal_size().context("failed to get terminal size")?;
        let (term_w_min, term_h_min) = ((asc.w as u32 * 2 + 4).clamp(0, u16::MAX.into()) as u16, 30);
        if term_w < term_w_min || term_h < term_h_min {
            printc!("&cWarning: Your terminal is too small ({term_w} * {term_h}).\n\
                     Please resize it to at least ({term_w_min} * {term_h_min}) for better experience.");
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

        let print_color_testing = |label: &str, mode: AnsiMode| {
            let label = format!("{label:^term_w$}", term_w = usize::from(term_w));
            let line = zip(gradient.iter(), label.chars()).fold(
                String::new(),
                |mut s, (&rgb_f32_color, t)| {
                    let rgb_u8_color = Srgb::<u8>::from_linear(rgb_f32_color);
                    let back = rgb_u8_color
                        .to_ansi_string(mode, ForegroundBackground::Background);
                    let fore = rgb_u8_color
                        .contrast_grayscale()
                        .to_ansi_string(AnsiMode::Ansi256, ForegroundBackground::Foreground);
                    write!(s, "{back}{fore}{t}").unwrap();
                    s
                },
            );
            printc!("{line}");
        };

        print_color_testing("8bit Color Testing", AnsiMode::Ansi256);
        print_color_testing("RGB Color Testing", AnsiMode::Rgb);

        println!();
        print_title_prompt(option_counter, "Which &bcolor system &ado you want to use?");
        println!("(If you can't see colors under \"RGB Color Testing\", please choose 8bit)\n");

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

        print_title_prompt(option_counter, "Is your terminal in &blight mode&~ or &4dark mode&~?");
        let choice = literal_input(
            "",
            TerminalTheme::VARIANTS,
            TerminalTheme::Dark.as_ref(),
            true,
            color_mode,
        )?;
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

    struct RawModeGuard {
        enabled: bool,
    }

    impl RawModeGuard {
        fn new() -> Result<Self> {
            Ok(Self { enabled: false })
        }

        fn enable(&mut self) -> Result<()> {
            if !self.enabled {
                enable_raw_mode().context("failed to enable terminal raw mode")?;
                self.enabled = true;
            }
            Ok(())
        }

        fn disable(&mut self) -> Result<()> {
            if self.enabled {
                disable_raw_mode().context("failed to disable terminal raw mode")?;
                self.enabled = false;
            }
            Ok(())
        }
    }

    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            if self.enabled {
                let _ = disable_raw_mode();
            }
        }
    }

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
        flags.push((
            preset.clone(),
            [name, flag.clone(), flag.clone(), flag],
            preset.as_ref().to_ascii_lowercase(),
        ));
    }

    // Calculate flags per row
    let (flags_per_row, rows_per_page) = {
        let (Width(term_w), Height(term_h)) = terminal_size().context("failed to get term size")?;
        let flags_per_row = (term_w / (spacing as u16 + 2)).clamp(0, u8::MAX.into()) as u8;
        let rows_per_page = (term_h.saturating_sub(13) / 5).clamp(1, u8::MAX.into()) as u8;
        (flags_per_row, rows_per_page)
    };
    let flags_per_page = usize::from(flags_per_row) * usize::from(rows_per_page);

    fn filter_flag_indices(query: &str, flags: &[(Preset, [String; 4], String)]) -> Vec<usize> {
        if query.is_empty() {
            return (0..flags.len()).collect();
        }

        let mut matched = flags
            .iter()
            .enumerate()
            .filter_map(|(idx, (_, _, preset_name))| {
                let position = preset_name.find(query)?;
                Some((idx, preset_name.starts_with(query), position))
            })
            .collect::<Vec<_>>();

        // Prefix matches are shown first, then other substring matches ordered by earliest index.
        matched.sort_by_key(|&(idx, is_prefix, position)| (!is_prefix, position, idx));
        matched.into_iter().map(|(idx, _, _)| idx).collect()
    }

    fn print_flag_row(row: &[&[String; 4]], color_mode: AnsiMode) -> Result<()> {
        for i in 0..4 {
            let mut line = Vec::new();
            for flag in row {
                line.push(&*flag[i]);
            }
            printc(line.join("  "), color_mode).context("failed to print line")?;
        }
        println!();
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

    let print_flag_page = |filtered_indices: &[usize],
                           page_num: usize,
                           filter: &str,
                           hint: Option<&str>|
     -> Result<()> {
        let num_pages = filtered_indices.len().div_ceil(flags_per_page).max(1);
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
        print_title_prompt(option_counter, "Let's choose a flag!");
        println!(
            "Available flag presets:\nPage: {page} of {num_pages}\n",
            page = page_num + 1
        );

        let start = page_num * flags_per_page;
        let end = (start + flags_per_page).min(filtered_indices.len());
        let mut visible_rows: usize = 0;
        if start >= end {
            println!("No presets matched this filter.\n");
        } else {
            for row in filtered_indices[start..end].chunks(usize::from(flags_per_row)) {
                let row = row
                    .iter()
                    .map(|&idx| &flags[idx].1)
                    .collect::<Vec<&[String; 4]>>();
                print_flag_row(&row, color_mode).context("failed to print flag row")?;
                visible_rows += 1;
            }
            println!();
        }
        // Keep the prompt anchored by reserving a full page worth of flag rows.
        for _ in visible_rows..usize::from(rows_per_page) {
            for _ in 0..5 {
                println!();
            }
        }

        println!(
            "Use arrow keys to go to the previous/next page. Type to filter and press Enter to select."
        );
        printc(
            format!(
                "Which {preset_default_colored} do you want to use? (default: {})",
                Preset::Rainbow.as_ref()
            ),
            color_mode,
        )
        .context("failed to print preset prompt")?;
        print!("> {filter}");
        io::stdout().flush().context("failed to flush preset prompt")?;

        if let Some(hint) = hint {
            println!("{hint}");
        }
        Ok(())
    };

    let preset: Preset;
    let color_profile;

    let mut page: usize = 0;
    let mut filter = String::new();
    let mut hint: Option<&str> = None;
    let mut raw_mode = RawModeGuard::new().context("failed to initialize raw input mode")?;
    loop {
        raw_mode
            .disable()
            .context("failed to disable raw mode for rendering")?;
        let filter_lower = filter.to_ascii_lowercase();
        let filtered_indices = filter_flag_indices(&filter_lower, &flags);
        let num_pages = filtered_indices.len().div_ceil(flags_per_page).max(1);
        page = page.min(num_pages - 1);

        print_flag_page(&filtered_indices, page, &filter, hint)
            .context("failed to print flag page")?;
        hint = None;

        raw_mode
            .enable()
            .context("failed to enable raw mode for key input")?;
        let event = event::read().context("failed to read keyboard event")?;
        let Event::Key(key) = event else {
            continue;
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }

        match key.code {
            KeyCode::Enter => {
                let selection = flags
                    .iter()
                    .find(|(_, _, name)| *name == filter_lower)
                    .map(|(preset, _, _)| *preset)
                    .or_else(|| filtered_indices.first().map(|&idx| flags[idx].0));
                preset = selection.unwrap_or(Preset::Rainbow);
                    break;
            },
            KeyCode::Left | KeyCode::Up => {
            page = (page + num_pages - 1) % num_pages;
            },
            KeyCode::Right | KeyCode::Down => {
                page = (page + 1) % num_pages;
            },
            KeyCode::Backspace => {
                filter.pop();
                page = 0;
            },
            KeyCode::Esc => {
                filter.clear();
                page = 0;
            },
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                raw_mode
                    .disable()
                    .context("failed to disable raw mode before interrupting")?;
                println!();
                return Err(anyhow::anyhow!("interrupted by user"));
            },
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                filter.push(c);
                page = 0;
            },
            _ => {},
        }
    }
    raw_mode
        .disable()
        .context("failed to disable raw mode after preset selection")?;

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
    printc(
        format!(
            "Which {preset_default_colored} do you want to use? {}\n",
            preset.as_ref()
        ),
        color_mode,
    )
    .context("failed to print preset selection summary")?;

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
        print_title_prompt(option_counter, "Let's adjust the color brightness!");
        println!(
            "The colors might be a little bit too {bright_dark} for {light_dark} mode.\n",
            bright_dark = match theme {
                TerminalTheme::Light => "bright",
                TerminalTheme::Dark => "dark",
            },
            light_dark = theme.as_ref()
        );

        let color_align = ColorAlignment::Horizontal;

        // Print cats
        {
            let (Width(term_w), _) = terminal_size().context("failed to get terminal size")?;
            let num_cols = (term_w / (test_ascii.w as u16 + 2)).clamp(1, u8::MAX.into()) as u8;
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
            println!("\nWhich brightness level looks the best? (Default: {default:.0}% for {light_dark} mode)",
                default = f32::from(default_lightness) * 100.0,
                light_dark = theme.as_ref()
            );
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
                    printc!("&cUnable to parse lightness value, please enter a lightness value such as 45%, .45, or 45");
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
    // 5. Choose Default or Small Logo

    // Calculate amount of row/column that can be displayed on screen
    let (ascii_per_row, ascii_rows) = {
        let (Width(term_w), Height(term_h)) = terminal_size().context("failed to get terminal size")?;
        let ascii_per_row = (term_w / (asc.w as u16 + 4)).clamp(1, u8::MAX.into()) as u8;
        let ascii_rows = (term_h.saturating_sub(8) / (asc.h as u16 + 1)).clamp(1, u8::MAX.into()) as u8;
        (ascii_per_row, ascii_rows)
    };

    // get distro string and convert it into the enum, neofetch friendly format, so we can check for small logos with the {distro}_small neofetch naming scheme.
    let tmp_dst = get_distro_name(backend).or_else(|_| get_distro_name(Backend::Neofetch)).context("failed to get distro name")?;
    let detected_dst = Some(distro.map_or_else(
        || {
            Distro::detect(&tmp_dst)
                .map_or("".to_string(), |d| format!("{:?}", d).to_lowercase())
        },
        |d| d.to_string().to_lowercase(),
    ));

    // in case someone specified {distro}_small already in the --distro arg
    let detected_dst_small_fmt = if !detected_dst.clone().unwrap().ends_with("_small") {
        format!("{}_small", detected_dst.unwrap()).to_lowercase()
    } else {
        detected_dst.unwrap()
    };
    
    let running_dst_sml = if Distro::detect(&detected_dst_small_fmt).is_some() {
        detected_dst_small_fmt
    } else {
        "".to_string()
    };

    
    // load ascii
    let small_asc = get_distro_ascii(Some(&running_dst_sml), backend).context("failed to get distro ascii")?;
    let small_asc = small_asc.to_normalized().context("failed to normalize ascii")?;
    
    let mut asc = asc;
    let mut logo_chosen: Option<String> = distro.cloned(); 
    
    if small_asc.lines != asc.lines && running_dst_sml != "" { 
        let ds_arrangements = [
            ("Default", asc.clone()),
            ("Small", small_asc.clone())
        ];   

        let arrangements: IndexMap<Cow<str>, NormalizedAsciiArt> =
            ds_arrangements.map(|(k, a)| (k.into(), a)).into();

        loop {
            clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;

            let asciis: Vec<Vec<String>> = arrangements
                .iter()
                .map(|(k, a)| {
                    let mut v: Vec<String> = a
                        .to_recolored(&ColorAlignment::Horizontal, &color_profile, color_mode, theme)
                        .context("failed to recolor ascii")?
                        .lines;
                        if k == "Small" {
                            // vertical center
                            let pad_len = (asc.h as usize - v.len()) / 2;
                            let mut pad_v: Vec<String> = vec![];
                            for _ in 0..pad_len {
                                pad_v.push("".to_string());
                            }
                            v.splice(0..0, pad_v.clone());
                            v.extend(pad_v);

                            let pad_diff = asc.h as usize - v.len();
                            v.extend(std::iter::repeat("".to_string()).take(pad_diff));
                            v.push(format!("{k:^asc_width$}", asc_width = usize::from(small_asc.w)));
                            return Ok(v);
                        }
                        v.push(format!("{k:^asc_width$}", asc_width = usize::from(asc.w)));
                        Ok(v)
                })
                .collect::<Result<_>>()?;

            // prints small logo w/ big logo
            for row in &asciis.into_iter().chunks(usize::from(ascii_per_row)) {
                
                let row: Vec<Vec<String>> = row.collect();
                
                for i in 0..usize::from(asc.h).checked_add(1).unwrap() {
                    let mut line = Vec::new();
                    for lines in &row {
                            line.push(&*lines[i]);
                    }
                    printc(line.join("                 "), color_mode).context("failed to print ascii line")?; 
                }

                println!();
            }

            print_title_prompt(option_counter, "Do you want the default logo, or the small logo?");
            let opts: Vec<Cow<str>> = ["default", "small"].map(Into::into).into();
            let choice = literal_input("Your choice?", &opts[..], "default", true, color_mode)
                .context("failed to ask for choice input")
                .context("failed to select logo type").context("failed to ask for choice input")?;
            
            if choice.to_lowercase() == "small" {
                logo_chosen = Some(running_dst_sml);
                asc = small_asc;
            }

            update_title(
                &mut title,
                &mut option_counter,
                "Selected logo type",
                choice.as_ref(),
            );

            break;
        }
    }

    //////////////////////////////
    // 6. Color arrangement

    let color_align: ColorAlignment;

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
        let mut preset_indices: Vec<PresetIndexedColor> = (0..color_profile.unique_colors().colors.len()).map(|x| (x as u8).into()).collect();
        while preset_indices.len() < slots.len() {
            preset_indices.extend_from_within(0..);
        }
        let preset_index_permutations: IndexSet<Vec<PresetIndexedColor>> = preset_indices
            .into_iter()
            .permutations(slots.len())
            .take(1000)
            .collect();
        let random_count = (ascii_per_row as usize * ascii_rows as usize).saturating_sub(arrangements.len()).clamp(0, u8::MAX.into()) as u8;
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
                printc(line.join("    "), color_mode).context("failed to print ascii line")?;
            }
            println!();
        }

        print_title_prompt(option_counter, "Let's choose a color arrangement!");
        println!("You can choose standard horizontal or vertical alignment, or use one of the random color schemes.\nYou can type \"roll\" to randomize again.\n");
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
        color_align = if choice == "horizontal" { ColorAlignment::Horizontal }
        else if choice == "vertical" { ColorAlignment::Vertical }
        else { 
            arrangements.into_iter()
                .find_map(|(k, ca)| {
                    if k.to_lowercase() == choice {
                        Some(ca)
                    } else {
                        None
                    }
                })
                .expect("selected color alignment is not valid") // TODO: it sould ask the user to retry instead of panicking
        };
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
    // 7. Select *fetch backend

    let select_backend = || -> Result<Backend> {
        clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
        print_title_prompt(option_counter, "Select a *fetch backend");

        // Check if fastfetch is installed
        let fastfetch_path = fastfetch_path().ok();

        // Check if macchina is installed
        #[cfg(feature = "macchina")]
        let macchina_path = macchina_path().unwrap_or(None);

        printc!("- &bneofetch&r: Written in bash, &nbest compatibility&r on Unix systems");
        printc!("- &bfastfetch&r: Written in C, &nbest performance&r {}",
            fastfetch_path
            .map(|path| format!("&a(Installed at {path})", path = path.display()))
            .unwrap_or_else(|| "&c(Not installed)".to_owned())
        );
        #[cfg(feature = "macchina")]
        printc!("- &bmacchina&r: Written in Rust, &nbest performance&r {}\n",
            macchina_path
            .map(|path| format!("&a(Installed at {path})", path = path.display()))
            .unwrap_or_else(|| "&c(Not installed)".to_owned())
        );

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

    //////////////////////////////
    // 8. Custom ASCII file
    let mut custom_ascii_path: Option<String> = None;
    clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
    let choice = literal_input(
        "Do you want to specify a custom ASCII file?",
        &["y", "n"],
        "n",
        true,
        color_mode,
    )
    .context("failed to ask for choice input")?;

    if choice == "y" {
        loop {
            let pth = input(Some("Path to custom ASCII file (must be UTF-8 encoded, empty to skip): "))?.trim().to_owned();
            if pth.is_empty() {
                printc!("&cNo path entered. Skipping custom ASCII file.");
                break;
            }

            let pth_buf = PathBuf::from(&pth);
            if !pth_buf.is_file() {
                printc!("&cError: File not found at {pth}");
                continue;
            }

            match fs::read_to_string(&pth_buf) {
                Ok(_) => {
                    custom_ascii_path = Some(pth);
                    update_title(
                        &mut title,
                        &mut option_counter,
                        "Custom ASCII file",
                        custom_ascii_path.as_ref().unwrap(),
                    );
                    break;
                }
                Err(e) => {
                    printc!("&cError: File is not UTF-8 encoded or an unexpected error occurred: {e}");
                    continue;
                }
            }
        }
    }

    // Create config
    clear_screen(Some(&title), color_mode, debug_mode).context("failed to clear screen")?;
    let config = Config {
        preset: preset.as_ref().to_string(),
        mode: color_mode,
        light_dark: Some(theme),
        auto_detect_light_dark: Some(det_bg.is_some()),
        lightness: Some(lightness),
        color_align,
        backend,
        args: None,
        distro: logo_chosen,
        pride_month_disable: false,
        custom_ascii_path,
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
