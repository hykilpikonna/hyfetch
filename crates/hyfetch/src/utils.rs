use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::{env, fs, io};

use anyhow::{anyhow, Context as _, Result};
use directories::ProjectDirs;
#[cfg(windows)]
use normpath::PathExt as _;
use tracing::debug;

pub fn get_cache_path() -> Result<PathBuf> {
    let path = ProjectDirs::from("", "", "hyfetch")
        .context("failed to get base dirs")?
        .cache_dir()
        .to_owned();
    Ok(path)
}

/// Reads a string from standard input. The trailing newline is stripped.
///
/// The prompt string, if given, is printed to standard output without a
/// trailing newline before reading input.
pub fn input<S>(prompt: Option<S>) -> Result<String>
where
    S: AsRef<str>,
{
    if let Some(prompt) = prompt {
        write!(io::stdout(), "{prompt}", prompt = prompt.as_ref())
            .and_then(|_| io::stdout().flush())
            .context("failed to write prompt to stdout")?;
    }

    io::stdin()
        .lines()
        .next()
        .unwrap_or_else(|| Ok(String::new()))
        .context("failed to read line from stdin")
}

/// Finds a command in `PATH`.
///
/// Returns the canonicalized / normalized absolute path of the command.
pub fn find_in_path<P>(program: P) -> Result<Option<PathBuf>>
where
    P: AsRef<Path>,
{
    let program = program.as_ref();

    // Only accept program name, i.e. a relative path with one component
    if program.parent() != Some(Path::new("")) {
        return Err(anyhow!("invalid command name {program:?}"));
    };

    let path_env = env::var_os("PATH").context("`PATH` env var is not set or invalid")?;

    for search_path in env::split_paths(&path_env) {
        let path = search_path.join(program);
        let path = find_file(&path)
            .with_context(|| format!("failed to check existence of file {path:?}"))?;
        if path.is_some() {
            return Ok(path);
        }
    }

    Ok(None)
}

/// Finds a file.
///
/// Returns the canonicalized / normalized absolute path of the file.
pub fn find_file<P>(path: P) -> Result<Option<PathBuf>>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(None);
        },
        Err(err) => {
            return Err(err).with_context(|| format!("failed to get metadata for {path:?}"));
        },
    };

    if !metadata.is_file() {
        debug!(?path, "path exists but is not a file");
        return Ok(None);
    }

    #[cfg(not(windows))]
    {
        path.canonicalize()
            .with_context(|| format!("failed to canonicalize path {path:?}"))
            .map(Some)
    }
    #[cfg(windows)]
    {
        path.normalize()
            .with_context(|| format!("failed to normalize path {path:?}"))
            .map(|p| Some(p.into()))
    }
}

pub fn process_command_status(status: &ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    let err = if let Some(code) = status.code() {
        anyhow!("child process exited with status code: {code}")
    } else {
        #[cfg(unix)]
        {
            anyhow!(
                "child process terminated by signal: {signal}",
                signal = status
                    .signal()
                    .expect("either one of status code or signal should be set")
            )
        }
        #[cfg(not(unix))]
        {
            unimplemented!("status code not expected to be `None` on non-Unix platforms")
        }
    };
    Err(err)
}

pub(crate) mod index_map_serde {
    use std::fmt;
    use std::hash::Hash;
    use std::marker::PhantomData;
    use std::str::FromStr;

    use indexmap::IndexMap;
    use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D, K, V>(deserializer: D) -> Result<IndexMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Eq + Hash + FromStr,
        K::Err: fmt::Display,
        V: Deserialize<'de>,
    {
        struct KeySeed<K> {
            k: PhantomData<K>,
        }

        impl<'de, K> DeserializeSeed<'de> for KeySeed<K>
        where
            K: FromStr,
            K::Err: fmt::Display,
        {
            type Value = K;

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_str(self)
            }
        }

        impl<'de, K> Visitor<'de> for KeySeed<K>
        where
            K: FromStr,
            K::Err: fmt::Display,
        {
            type Value = K;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                K::from_str(s).map_err(de::Error::custom)
            }
        }

        struct MapVisitor<K, V> {
            k: PhantomData<K>,
            v: PhantomData<V>,
        }

        impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
        where
            K: Eq + Hash + FromStr,
            K::Err: fmt::Display,
            V: Deserialize<'de>,
        {
            type Value = IndexMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<A>(self, mut input: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut map = IndexMap::new();
                while let Some((k, v)) =
                    input.next_entry_seed(KeySeed { k: PhantomData }, PhantomData)?
                {
                    map.insert(k, v);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(MapVisitor {
            k: PhantomData,
            v: PhantomData,
        })
    }
}
