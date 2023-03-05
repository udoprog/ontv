use std::fmt;
use std::fs;
use std::future::Future;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

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

    fn serialize_pretty<O, T>(self, f: &mut O, data: &T) -> Result<()>
    where
        O: Write,
        T: Serialize,
    {
        match self {
            Format::Yaml => {
                serde_yaml::to_writer(&mut *f, data)?;
                f.write_all(&[b'\n'])?;
            }
            Format::Json => {
                serde_json::to_writer_pretty(&mut *f, data)?;
                f.write_all(&[b'\n'])?;
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

/// Save series to the given path.
pub(crate) fn save_pretty<P, I>(
    what: &'static str,
    path: &P,
    data: I,
) -> impl Future<Output = Result<()>>
where
    P: ?Sized + AsRef<Path>,
    I: 'static + Send + Serialize,
{
    let path = Box::<Path>::from(path.as_ref());
    tracing::debug!("saving {what}: {}", path.display());

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mode = Format::from_path(&path)
            .with_context(|| anyhow!("{}: unsupported mode", path.display()))?;

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        tracing::trace!("writing {what}: {}", f.path().display());

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

    async move {
        let output: Result<()> = task.await?;
        output
    }
}

/// Save array to the given paths.
pub(crate) fn save_array_fallback<P, I, const N: usize>(
    what: &'static str,
    paths: [&P; N],
    data: I,
) -> impl Future<Output = Result<()>>
where
    P: ?Sized + AsRef<Path>,
    I: 'static + Clone + Send + IntoIterator,
    I::Item: Serialize,
{
    let paths = std::array::from_fn::<_, N, _>(|index| Box::<Path>::from(paths[index].as_ref()));

    async move {
        let mut it = paths.into_iter();
        let last = it.next_back();

        for path in it {
            save_array(what, &path, data.clone()).await?;
        }

        if let Some(path) = last {
            save_array(what, &path, data).await?;
        }

        Ok(())
    }
}

/// Save series to the given path.
pub(crate) fn save_array<P, I>(
    what: &'static str,
    path: &P,
    data: I,
) -> impl Future<Output = Result<()>>
where
    P: ?Sized + AsRef<Path>,
    I: 'static + Send + IntoIterator,
    I::Item: Serialize,
{
    let path = path.as_ref();

    tracing::trace!("saving {what}: {}", path.display());

    let path = Box::<Path>::from(path.as_ref());

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mode = Format::from_path(&path)
            .with_context(|| anyhow!("{}: unsupported mode", path.display()))?;

        let f = tempfile::NamedTempFile::new_in(dir)?;
        tracing::trace!("writing {what}: {}", f.path().display());
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

    async move {
        let output: Result<()> = task.await?;
        output
    }
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

        if !m.is_file() {
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

        if !matches!(path.extension().and_then(|e| e.to_str()), Some("json")) {
            continue;
        }

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
pub(crate) fn load_array_fallback<P, T, const N: usize>(
    paths: [&P; N],
) -> Result<Option<(usize, Format, Vec<T>)>>
where
    P: ?Sized + AsRef<Path>,
    T: DeserializeOwned,
{
    for (index, path) in paths.into_iter().enumerate().rev() {
        if let Some((format, array)) = load_array(path)? {
            return Ok(Some((index, format, array)));
        }
    }

    Ok(None)
}

/// Load a simple array from a file.
pub(crate) fn load_array<P, T>(path: &P) -> Result<Option<(Format, Vec<T>)>>
where
    T: DeserializeOwned,
    P: ?Sized + AsRef<Path>,
{
    let path = path.as_ref();

    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::from(e)).with_context(|| anyhow!("{}", path.display())),
    };

    let format = Format::from_path(&path)
        .with_context(|| anyhow!("{}: unsupported file extension", path.display()))?;

    let array = format
        .deserialize_array(f)
        .with_context(|| anyhow!("{}", path.display()))?;

    Ok(Some((format, array)))
}
