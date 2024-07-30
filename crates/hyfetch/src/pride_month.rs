use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::num::{NonZeroU16, NonZeroUsize, Wrapping};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{cmp, thread};

use anyhow::{anyhow, Context as _, Result};
use crossterm::execute;
use crossterm::terminal::{
    BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
};
use palette::blend::Blend as _;
use palette::{LinSrgba, Srgb, WithAlpha as _};
use strum::VariantArray as _;
use terminal_size::{terminal_size, Height, Width};

use crate::color_util::{color, ForegroundBackground, ToAnsiString as _};
use crate::neofetch_util::ascii_size;
use crate::presets::Preset;
use crate::types::AnsiMode;

const TEXT_ASCII: &str = r"
.======================================================.
| .  .              .__       .     .  .       , .   | |
| |__| _.._ ._   .  [__)._.* _| _   |\/| _ ._ -+-|_  | |
| |  |(_][_)[_)\_|  |   [  |(_](/,  |  |(_)[ ) | [ ) * |
|        |  |  ._|                                     |
'======================================================'
";

const TEXT_ASCII_SMALL: &str = r"
.====================.
| Happy Pride Month! |
'===================='
";

const NOTICE: &str = "Press enter to continue";

pub fn start_animation(color_mode: AnsiMode) -> Result<()> {
    let (w, h) = {
        let (Width(w), Height(h)) = terminal_size().context("failed to get terminal size")?;
        let w: NonZeroU16 = w.try_into().context("terminal width should not be 0")?;
        let h: NonZeroU16 = h.try_into().context("terminal height should not be 0")?;
        (w, h)
    };

    let text = &TEXT_ASCII[1..TEXT_ASCII.len().checked_sub(1).unwrap()];
    let (text_width, text_height) =
        ascii_size(text).expect("text ascii should have valid width and height");
    let (text, text_width, text_height) = {
        const TEXT_BORDER_WIDTH: u16 = 2;
        const NOTICE_BORDER_WIDTH: u16 = 1;
        const VERTICAL_MARGIN: u16 = 1;
        let notice_w = NOTICE.len();
        let notice_w: u8 = notice_w
            .try_into()
            .expect("`NOTICE` width should fit in `u8`");
        let notice_h = NOTICE.lines().count();
        let notice_h: u8 = notice_h
            .try_into()
            .expect("`NOTICE` height should fit in `u8`");
        let term_w_min = cmp::max(
            u16::from(text_width)
                .checked_add(TEXT_BORDER_WIDTH.checked_mul(2).unwrap())
                .unwrap(),
            u16::from(notice_w)
                .checked_add(NOTICE_BORDER_WIDTH.checked_mul(2).unwrap())
                .unwrap(),
        );
        let term_h_min = u16::from(text_height)
            .checked_add(notice_h.into())
            .unwrap()
            .checked_add(VERTICAL_MARGIN.checked_mul(2).unwrap())
            .unwrap();
        if w.get() >= term_w_min && h.get() >= term_h_min {
            (text, text_width, text_height)
        } else {
            let text = &TEXT_ASCII_SMALL[1..TEXT_ASCII_SMALL.len().checked_sub(1).unwrap()];
            let (text_width, text_height) =
                ascii_size(text).expect("text ascii should have valid width and height");
            let term_w_min = cmp::max(
                u16::from(text_width)
                    .checked_add(TEXT_BORDER_WIDTH.checked_mul(2).unwrap())
                    .unwrap(),
                u16::from(notice_w)
                    .checked_add(NOTICE_BORDER_WIDTH.checked_mul(2).unwrap())
                    .unwrap(),
            );
            let term_h_min = u16::from(text_height)
                .checked_add(notice_h.into())
                .unwrap()
                .checked_add(VERTICAL_MARGIN.checked_mul(2).unwrap())
                .unwrap();
            if w.get() < term_w_min || h.get() < term_h_min {
                return Err(anyhow!(
                    "terminal size should be at least ({term_w_min} * {term_h_min})"
                ));
            }
            (text, text_width, text_height)
        }
    };
    let text_lines: Vec<&str> = text.lines().collect();

    const BLOCKS: u8 = 9;
    let block_width: NonZeroU16 = w
        .get()
        .div_euclid(u16::from(BLOCKS))
        .try_into()
        .with_context(|| format!("terminal width should be at least {BLOCKS}"))?;

    let text_start_y = h
        .get()
        .div_euclid(2)
        .checked_sub((text_height / 2).into())
        .unwrap();
    let text_end_y = text_start_y.checked_add(text_height.into()).unwrap();
    let text_start_x = w
        .get()
        .div_euclid(2)
        .checked_sub((text_width / 2).into())
        .unwrap();
    let text_end_x = text_start_x.checked_add(text_width.into()).unwrap();

    let notice_start_x = w
        .get()
        .checked_sub(
            u8::try_from(NOTICE.len())
                .expect("`NOTICE` length should fit in `u8`")
                .into(),
        )
        .unwrap()
        .checked_sub(1)
        .unwrap();
    let notice_end_x = w.get().checked_sub(1).unwrap();
    let notice_y = h.get().checked_sub(1).unwrap();

    // Add every preset to colors
    let colors: Vec<Srgb<u8>> = Preset::VARIANTS
        .iter()
        .flat_map(|p| p.color_profile().colors)
        .collect();

    let fg: Srgb<u8> = "#FFE09B"
        .parse()
        .expect("foreground color hex should be valid");
    let black = LinSrgba::new(0.0, 0.0, 0.0, 0.5);

    let draw_frame = |frame: usize| -> Result<()> {
        execute!(io::stdout(), BeginSynchronizedUpdate)
            .context("failed to begin synchronized update")?;

        let mut buf = String::new();

        // Loop over the height
        for y in 0..h.get() {
            // Print the starting color
            write!(
                buf,
                "{bg}{fg}",
                bg = colors[frame
                    .wrapping_add(y.into())
                    .div_euclid(block_width.get().into())
                    .rem_euclid(colors.len())]
                .to_ansi_string(color_mode, ForegroundBackground::Background),
                fg = fg.to_ansi_string(color_mode, ForegroundBackground::Foreground)
            )
            .unwrap();

            // Loop over the width
            for x in 0..w.get() {
                let idx = frame
                    .wrapping_add(x.into())
                    .wrapping_add(y.into())
                    .wrapping_add_signed((2.0 * (y as f64 + 0.5 * frame as f64).sin()) as isize);
                let y_text = text_start_y <= y && y < text_end_y;

                let border = 1u16
                    .checked_add(
                        if y == text_start_y || y == text_end_y.checked_sub(1).unwrap() {
                            0
                        } else {
                            1
                        },
                    )
                    .unwrap();
                let text_bounds_x1 = text_start_x
                    .checked_sub(border)
                    .expect("`text_start_x - border` should not underflow `u16`");
                let text_bounds_x2 = text_end_x
                    .checked_add(border)
                    .expect("`text_end_x + border` should not overflow `u16`");
                let notice_bounds_x1 = notice_start_x
                    .checked_sub(1)
                    .expect("`notice_start_x - 1` should not underflow `u16`");
                let notice_bounds_x2 = notice_end_x
                    .checked_add(1)
                    .expect("`notice_end_x + 1` should not overflow `u16`");

                // If it's a switching point
                if idx.rem_euclid(NonZeroUsize::from(block_width).get()) == 0
                    || x == text_bounds_x1
                    || x == text_bounds_x2
                    || x == notice_bounds_x1
                    || x == notice_bounds_x2
                {
                    // Print the color at the current frame
                    let ci = idx
                        .div_euclid(NonZeroUsize::from(block_width).get())
                        .rem_euclid(colors.len());
                    let c = colors[ci];
                    if (y_text && (text_bounds_x1 <= x) && (x < text_bounds_x2))
                        || (y == notice_y && notice_bounds_x1 <= x && x < notice_bounds_x2)
                    {
                        let c: LinSrgba = c.with_alpha(1.0).into_linear();
                        let c = Srgb::<u8>::from_linear(c.overlay(black).without_alpha());
                        write!(
                            buf,
                            "{bg}",
                            bg = c.to_ansi_string(color_mode, ForegroundBackground::Background),
                        )
                        .unwrap();
                    } else {
                        write!(
                            buf,
                            "{bg}",
                            bg = c.to_ansi_string(color_mode, ForegroundBackground::Background),
                        )
                        .unwrap();
                    }
                }

                // If text should be printed, print text
                if y_text && text_start_x <= x && x < text_end_x {
                    write!(
                        buf,
                        "{text_char}",
                        text_char = text_lines[usize::from(y.checked_sub(text_start_y).unwrap())]
                            .chars()
                            .nth(usize::from(x.checked_sub(text_start_x).unwrap()))
                            .unwrap(),
                    )
                    .unwrap();
                } else if y == notice_y && notice_start_x <= x && x < notice_end_x {
                    write!(
                        buf,
                        "{notice_char}",
                        notice_char = NOTICE
                            .chars()
                            .nth(usize::from(x.checked_sub(notice_start_x).unwrap()))
                            .unwrap(),
                    )
                    .unwrap();
                } else {
                    write!(buf, " ").unwrap();
                }
            }

            // New line if it isn't the last line
            if y != h.get().checked_sub(1).unwrap() {
                writeln!(
                    buf,
                    "{reset}",
                    reset = color("&r", color_mode).expect("reset should be valid"),
                )
                .unwrap();
            }
        }

        {
            let mut stdout = io::stdout().lock();
            write!(stdout, "{buf}")
                .and_then(|_| stdout.flush())
                .context("failed to write to stdout")?;
        }

        execute!(io::stdout(), EndSynchronizedUpdate)
            .context("failed to end synchronized update")?;

        Ok(())
    };

    let key_pressed = Arc::new(AtomicBool::new(false));

    // TODO: use non-blocking I/O; no need for another thread
    let _handle = thread::spawn({
        let key_pressed = Arc::clone(&key_pressed);
        move || {
            loop {
                match io::stdin().lines().next() {
                    Some(Ok(_)) => {
                        key_pressed.store(true, Ordering::Release);
                        break;
                    },
                    Some(Err(err)) => {
                        eprintln!("failed to read line from stdin: {err}");
                    },
                    None => {
                        // EOF
                    },
                }
            }
        }
    });

    let mut frame: Wrapping<usize> = Wrapping(0);

    const SPEED: u8 = 2;
    let frame_delay = Duration::from_secs_f32(1.0 / 25.0);

    execute!(io::stdout(), EnterAlternateScreen).context("failed to enter alternate screen")?;

    loop {
        draw_frame(frame.0)?;
        frame += usize::from(SPEED);
        thread::sleep(frame_delay);

        if key_pressed.load(Ordering::Acquire) {
            break;
        }
    }

    execute!(io::stdout(), LeaveAlternateScreen).context("failed to leave alternate screen")?;

    Ok(())
}
