use std::fmt;
use std::fs;
use std::future::Future;
use std::io;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::service::paths;

pub(crate) enum Format {
    Yaml,
    Json,
}

impl Format {
    /// Get a mode from a path.
    fn from_path<P>(path: &P) -> Option<Format>
    where
        P: ?Sized + AsRef<Path>,
    {
        match path.as_ref().extension().and_then(|e| e.to_str()) {
            Some("json") => Some(Self::Json),
            Some("yaml") => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Deserialize an array under the current mode.
    pub(crate) fn deserialize_array<T, R>(&self, f: R) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned,
        R: Read,
    {
        /// Load an array from the given reader line-by-line.
        fn from_json<T, R>(input: R) -> Result<Vec<T>>
        where
            T: DeserializeOwned,
            R: Read,
        {
            use std::io::{BufRead, BufReader};

            let mut output = Vec::new();

            for line in BufReader::new(input).lines() {
                let line = line?;
                let line = line.trim();

                if line.starts_with('#') || line.is_empty() {
                    continue;
                }

                output.push(serde_json::from_str(line)?);
            }

            Ok(output)
        }

        match self {
            Format::Yaml => {
                let mut array = Vec::new();

                for doc in serde_yaml::Deserializer::from_reader(f) {
                    array.push(T::deserialize(doc)?);
                }

                Ok(array)
            }
            Format::Json => from_json(f),
        }
    }

    /// Start writing an array.
    fn start_array<O>(self, output: &mut O) -> SerializeArray<'_, O> {
        SerializeArray {
            count: 0,
            mode: self,
            output,
        }
    }

    /// Deserialize using the current format.
    fn deserialize<T>(&self, bytes: &[u8]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match self {
            Format::Yaml => Ok(serde_yaml::from_slice(bytes)?),
            Format::Json => Ok(serde_json::from_slice(bytes)?),
        }
    }

    fn serialize_pretty<O, T>(self, f: &mut O, data: &T) -> Result<()>
    where
        O: Write,
        T: Serialize,
    {
        match self {
            Format::Yaml => {
                serde_yaml::to_writer(&mut *f, data)?;
                f.write_all(b"\n")?;
            }
            Format::Json => {
                serde_json::to_writer_pretty(&mut *f, data)?;
                f.write_all(b"\n")?;
            }
        }

        Ok(())
    }
}

struct SerializeArray<'a, O> {
    count: usize,
    mode: Format,
    output: &'a mut O,
}

impl<O> SerializeArray<'_, O>
where
    O: Write,
{
    fn serialize_item<T>(&mut self, item: &T) -> Result<()>
    where
        T: Serialize,
    {
        match self.mode {
            Format::Yaml => {
                if self.count > 0 {
                    self.output.write_all(b"---\n")?;
                }

                serde_yaml::to_writer(&mut *self.output, item)?;
            }
            Format::Json => {
                serde_json::to_writer(&mut *self.output, item)?;
                self.output.write_all(b"\n")?;
            }
        }

        self.count += 1;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        Ok(())
    }
}

/// Load configuration file.
pub(crate) fn load<T>(path: &paths::Candidate) -> Result<Option<(Format, T)>>
where
    T: DeserializeOwned,
{
    for path in path.read() {
        let Some(format) = Format::from_path(path) else {
            continue;
        };

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };

        let output = format.deserialize(&bytes)?;
        return Ok(Some((format, output)));
    }

    Ok(None)
}

/// Save pretty.
pub(crate) async fn save_pretty<T>(
    what: &'static str,
    path: &paths::Candidate,
    data: T,
) -> Result<()>
where
    T: 'static + Send + Serialize,
{
    save_pretty_inner(what, path.as_ref(), data).await?;

    for path in path.remainder() {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

async fn save_pretty_inner<P, T>(what: &'static str, path: &P, data: T) -> Result<()>
where
    P: ?Sized + AsRef<Path>,
    T: 'static + Send + Serialize,
{
    let path = Box::<Path>::from(path.as_ref());
    tracing::debug!(path = path.display().to_string(), what, "Saving");

    let task = tokio::task::spawn_blocking(move || {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: Missing parent directory for {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mode = Format::from_path(&path)
            .with_context(|| anyhow!("{}: unsupported mode", path.display()))?;

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        tracing::trace!(what, path = f.path().display().to_string(), "Writing");

        mode.serialize_pretty(&mut f, &data)?;
        let (mut f, temp_path) = f.keep()?;
        f.flush()?;
        drop(f);

        tracing::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    task.await?
}

/// Save array to the given paths.
pub(crate) async fn save_array<I>(
    what: &'static str,
    path: &paths::Candidate,
    data: I,
) -> Result<()>
where
    I: 'static + Send + IntoIterator,
    I::Item: Serialize,
{
    save_array_inner(what, path.as_ref(), data).await?;

    for path in path.remainder() {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

/// Save series to the given path.
fn save_array_inner<P, I>(what: &'static str, path: P, data: I) -> impl Future<Output = Result<()>>
where
    P: AsRef<Path>,
    I: 'static + Send + IntoIterator,
    I::Item: Serialize,
{
    let path = path.as_ref();

    tracing::trace!(what, path = path.display().to_string(), "Saving");

    let path = Box::<Path>::from(path);

    let task = tokio::task::spawn_blocking(move || {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mode = Format::from_path(&path)
            .with_context(|| anyhow!("{}: unsupported mode", path.display()))?;

        let f = tempfile::NamedTempFile::new_in(dir)?;
        tracing::trace!(what, path = f.path().display().to_string(), "Writing");
        let mut f = BufWriter::new(f);

        let mut writer = mode.start_array(&mut f);

        for line in data {
            writer.serialize_item(&line)?;
        }

        writer.finish()?;
        let (mut f, temp_path) = f.into_inner()?.keep()?;
        f.flush()?;
        drop(f);

        tracing::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    async move { task.await? }
}

/// Load all episodes found on the given paths.
pub(crate) fn load_directory<P, I, T>(path: &P) -> Result<Option<Vec<(I, Format, Vec<T>)>>>
where
    P: ?Sized + AsRef<Path>,
    I: FromStr,
    I::Err: fmt::Display,
    T: DeserializeOwned,
{
    let d = match fs::read_dir(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut output = Vec::new();

    for e in d {
        let e = e?;

        let m = e.metadata()?;

        if !m.is_file() || m.len() == 0 {
            continue;
        }

        let path = e.path();

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let mode = match Format::from_path(&path) {
            Some(value) => value,
            None => continue,
        };

        let Ok(id) = stem.parse() else {
            continue;
        };

        let f = std::fs::File::open(&path)?;
        let value = mode.deserialize_array(f)?;
        output.push((id, mode, value));
    }

    Ok(Some(output))
}

/// Load an array from one of several locations.
pub(crate) fn load_array<T>(path: &paths::Candidate) -> Result<Option<(Format, Vec<T>)>>
where
    T: DeserializeOwned,
{
    for path in path.read() {
        if let Some(output) = load_array_inner(path)? {
            return Ok(Some(output));
        }
    }

    Ok(None)
}

/// Load a simple array from a file.
fn load_array_inner<P, T>(path: P) -> Result<Option<(Format, Vec<T>)>>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let path = path.as_ref();

    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::from(e)).with_context(|| anyhow!("{}", path.display())),
    };

    let m = f.metadata()?;

    if m.len() == 0 {
        return Ok(None);
    }

    let format = Format::from_path(&path)
        .with_context(|| anyhow!("{}: unsupported file extension", path.display()))?;

    let array = format
        .deserialize_array(f)
        .with_context(|| anyhow!("{}", path.display()))?;

    Ok(Some((format, array)))
}
