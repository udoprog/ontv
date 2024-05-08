use std::path::Path;

#[derive(Clone)]
pub(crate) struct Candidate {
    json: Box<Path>,
    yaml: Box<Path>,
}

impl Candidate {
    pub(crate) fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        Self {
            json: path.to_owned().with_extension("json").into(),
            yaml: path.to_owned().with_extension("yaml").into(),
        }
    }

    /// Display implementation for back path.
    pub(crate) fn display(&self) -> std::path::Display<'_> {
        self.yaml.display()
    }

    /// All paths.
    pub(crate) fn all(&self) -> [&Path; 2] {
        self.read()
    }

    /// Read paths.
    pub(crate) fn read(&self) -> [&Path; 2] {
        [self.json.as_ref(), self.yaml.as_ref()]
    }

    /// Remainder paths.
    pub(crate) fn remainder(&self) -> [&Path; 1] {
        [self.json.as_ref()]
    }
}

impl AsRef<Path> for Candidate {
    fn as_ref(&self) -> &Path {
        &self.yaml
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

pub(crate) struct Paths {
    pub(crate) lock: tokio::sync::Mutex<()>,
    pub(crate) config: Candidate,
    pub(crate) sync: Candidate,
    pub(crate) remotes: Candidate,
    pub(crate) images: Box<Path>,
    pub(crate) series: Candidate,
    pub(crate) movies: Candidate,
    pub(crate) watched: Candidate,
    pub(crate) pending: Candidate,
    pub(crate) episodes: Directory,
    pub(crate) seasons: Directory,
}

impl Paths {
    /// Construct a new collection of paths.
    pub(crate) fn new(config: &Path, cache: &Path) -> Self {
        Self {
            lock: tokio::sync::Mutex::new(()),
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
            images: cache.join("images").into(),
        }
    }
}
