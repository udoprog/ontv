use std::collections::{BTreeSet, HashMap};

use crate::database::iter::Iter;
use crate::model::{Movie, MovieId};

#[derive(Default)]
pub(crate) struct Database {
    // Movie indexed by id.
    data: HashMap<MovieId, Movie>,
    // Movie indexed by name.
    by_name: BTreeSet<(String, MovieId)>,
}

impl Database {
    /// Get a movie immutably.
    pub(crate) fn get(&self, id: &MovieId) -> Option<&Movie> {
        self.data.get(id)
    }

    /// Get a movie mutably.
    pub(crate) fn get_mut(&mut self, id: &MovieId) -> Option<&mut Movie> {
        self.data.get_mut(id)
    }

    /// Remove the movie with the given identifier.
    pub(crate) fn remove(&mut self, id: &MovieId) -> Option<Movie> {
        let movie = self.data.remove(id)?;
        let _ = self.by_name.remove(&(movie.title.clone(), movie.id));
        Some(movie)
    }

    /// Insert the given movie.
    pub(crate) fn insert(&mut self, movie: Movie) {
        self.by_name.insert((movie.title.clone(), movie.id));
        self.data.insert(movie.id, movie);
    }

    /// Iterate over all movies in the database in some order.
    pub(crate) fn iter_by_name(&self) -> impl DoubleEndedIterator<Item = &Movie> {
        Iter::new(self.by_name.iter().map(|(_, key)| key), &self.data)
    }

    /// Iterate over all movies in the database in some random.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &Movie> {
        self.data.values()
    }

    /// Export movie data.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Movie> + 'static + use<> {
        let mut out = Vec::with_capacity(self.by_name.len());

        for (_, id) in &self.by_name {
            if let Some(movie) = self.data.get(id) {
                out.push(movie.clone());
            }
        }

        out
    }
}
