use core::iter::{empty, once};

use std::path::{Display, Path, PathBuf};

#[derive(Clone)]
pub(crate) struct Candidate {
    path: PathBuf,
}

impl Candidate {
    pub(crate) fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        Self {
            path: path.with_extension("yaml"),
        }
    }

    /// Display implementation for back path.
    pub(crate) fn display(&self) -> Display<'_> {
        self.path.display()
    }

    /// All paths.
    pub(crate) fn all(&self) -> impl IntoIterator<Item: AsRef<Path>> + '_ {
        once(&self.path)
    }

    /// Read paths.
    pub(crate) fn read(&self) -> impl IntoIterator<Item: AsRef<Path>> + '_ {
        once(&self.path)
    }

    /// Remainder paths.
    pub(crate) fn remainder(&self) -> impl IntoIterator<Item: AsRef<Path>> + '_ {
        empty::<PathBuf>()
    }
}

impl AsRef<Path> for Candidate {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

pub(crate) struct Directory {
    pub(crate) path: Box<Path>,
}

impl Directory {
    /// Join a directory path.
    pub(crate) fn join<P>(&self, path: P) -> Candidate
    where
        P: AsRef<Path>,
    {
        Candidate::new(self.path.join(path))
    }
}

impl AsRef<Path> for Directory {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.path.as_ref()
    }
}

/// Collection of application paths.
pub struct Paths {
    pub(crate) config: Candidate,
    pub(crate) sync: Candidate,
    pub(crate) remotes: Candidate,
    pub(crate) series: Candidate,
    pub(crate) movies: Candidate,
    pub(crate) watched: Candidate,
    pub(crate) pending: Candidate,
    pub(crate) episodes: Directory,
    pub(crate) seasons: Directory,
    pub(crate) db: Box<Path>,
    pub(crate) images: Box<Path>,
}

impl Paths {
    /// Construct a new collection of paths.
    pub(crate) fn new(config: &Path, cache: &Path) -> Self {
        Self {
            config: Candidate::new(config.join("config")),
            sync: Candidate::new(config.join("sync")),
            remotes: Candidate::new(config.join("remotes")),
            series: Candidate::new(config.join("series")),
            movies: Candidate::new(config.join("movies")),
            watched: Candidate::new(config.join("watched")),
            pending: Candidate::new(config.join("pending")),
            episodes: Directory {
                path: config.join("episodes").into(),
            },
            seasons: Directory {
                path: config.join("seasons").into(),
            },
            db: cache.join("ontv.sql").into(),
            images: cache.join("images").into(),
        }
    }
}
