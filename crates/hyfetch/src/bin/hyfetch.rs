use std::fmt;
use std::fs::{self, File};
use std::io::{self, ErrorKind, IsTerminal, Read};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Datelike;
use directories::ProjectDirs;
use hyfetch::cli_options::options;
use hyfetch::models::Config;
use hyfetch::neofetch_util::get_distro_ascii;
use tracing::debug;

fn main() -> Result<()> {
    let options = options().run();

    init_tracing_subsriber(options.debug).context("Failed to init tracing subscriber")?;

    debug!(?options, "CLI options");

    // TODO

    if options.test_print {
        println!(
            "{}",
            get_distro_ascii(options.distro.as_ref()).context("Failed to get distro ascii")?
        );
        return Ok(());
    }

    // TODO

    let config = if options.config {
        create_config(options.config_file).context("Failed to create config")?
    } else if let Some(config) =
        read_config(&options.config_file).context("Failed to read config")?
    {
        config
    } else {
        create_config(options.config_file).context("Failed to create config")?
    };

    let now = chrono::Local::now();
    let cache_path = ProjectDirs::from("", "", "hyfetch")
        .context("Failed to get base dirs")?
        .cache_dir()
        .to_owned();
    let june_path = cache_path.join(format!("animation-displayed-{}", now.year()));
    let show_pride_month =
        options.june || now.month() == 6 && !june_path.is_file() && io::stdout().is_terminal();

    if show_pride_month && !config.pride_month_disable {
        // TODO
        // pride_month.start_animation();
        println!();
        println!("Happy pride month!");
        println!("(You can always view the animation again with `hyfetch --june`)");
        println!();

        if !june_path.is_file() {
            fs::create_dir_all(cache_path).context("Failed to create cache dir")?;
            File::create(&june_path)
                .with_context(|| format!("Failed to create file {june_path:?}"))?;
        }
    }

    // TODO

    Ok(())
}

/// Reads config from file.
///
/// Returns `None` if the config file does not exist.
#[tracing::instrument(level = "debug")]
fn read_config<P>(path: P) -> Result<Option<Config>>
where
    P: AsRef<Path> + fmt::Debug,
{
    let path = path.as_ref();

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Ok(None);
        },
        Err(err) => {
            return Err(err).with_context(|| format!("Failed to open {path:?}"));
        },
    };

    let mut buf = String::new();

    file.read_to_string(&mut buf)
        .with_context(|| format!("Failed to read {path:?}"))?;

    let deserializer = &mut serde_json::Deserializer::from_str(&buf);
    let config: Config = serde_path_to_error::deserialize(deserializer)
        .with_context(|| format!("Failed to parse {path:?}"))?;

    debug!(?config, "read config");

    Ok(Some(config))
}

/// Creates config interactively.
///
/// The config is automatically stored to file.
#[tracing::instrument(level = "debug")]
fn create_config<P>(path: P) -> Result<Config>
where
    P: AsRef<Path> + fmt::Debug,
{
    todo!()
}

fn init_tracing_subsriber(debug: bool) -> Result<()> {
    use std::env;
    use std::str::FromStr;

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
        let targets = if debug {
            targets.with_target(env!("CARGO_CRATE_NAME"), Level::DEBUG)
        } else {
            targets
        };
        subscriber.with(targets)
    };

    subscriber
        .try_init()
        .context("Failed to set the global default subscriber")
}
