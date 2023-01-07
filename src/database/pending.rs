use crate::model::{Pending, SeriesId};

pub(crate) struct Index(usize);

#[derive(Default)]
pub(crate) struct Database {
    data: Vec<Pending>,
}

impl Database {
    /// Export data from the database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Pending> {
        self.data.clone()
    }

    /// Get a stable position for the pending episode related to the given series.
    ///
    /// The position will only be stable for as long as the database hasn't been modified.
    #[inline]
    pub(crate) fn position_for_series(&self, id: &SeriesId) -> Option<Index> {
        self.data.iter().position(|p| p.series == *id).map(Index)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: Index) -> Option<&mut Pending> {
        self.data.get_mut(index.0)
    }

    #[inline]
    pub(crate) fn sort(&mut self) {
        self.data.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Remove pending by predicate.
    #[inline]
    pub(crate) fn remove_by<P>(&mut self, mut predicate: P)
    where
        P: FnMut(&Pending) -> bool,
    {
        self.data.retain(move |p| !predicate(p))
    }

    /// Iterate immutably over pending entries in timestamp order.
    #[inline]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Pending> {
        self.data.iter()
    }

    /// Iterate mutably over data.
    #[inline]
    pub(crate) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Pending> {
        self.data.iter_mut()
    }

    /// Get pending by series.
    #[inline]
    pub(crate) fn by_series(&self, series_id: &SeriesId) -> Option<&Pending> {
        self.data.iter().find(|p| p.series == *series_id)
    }

    /// Get mutably by series.
    #[inline]
    pub(crate) fn get_mut_by_series(&mut self, series_id: &SeriesId) -> Option<&mut Pending> {
        self.data.iter_mut().find(|p| p.series == *series_id)
    }
}

impl Extend<Pending> for Database {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Pending>,
    {
        let before = self.data.len();
        self.data.extend(iter);

        if self.data.len() != before {
            self.sort();
        }
    }
}
