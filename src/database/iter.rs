use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

pub(crate) trait Map {
    type Key;
    type Value;

    fn get(&self, key: &Self::Key) -> Option<&Self::Value>;
}

impl<K, V> Map for BTreeMap<K, V>
where
    K: Eq + Ord,
{
    type Key = K;
    type Value = V;

    #[inline]
    fn get(&self, key: &Self::Key) -> Option<&Self::Value> {
        BTreeMap::get(self, key)
    }
}

impl<K, V> Map for HashMap<K, V>
where
    K: Eq + Hash,
{
    type Key = K;
    type Value = V;

    #[inline]
    fn get(&self, key: &Self::Key) -> Option<&Self::Value> {
        HashMap::get(self, key)
    }
}

pub(crate) struct Iter<'a, I, M> {
    iter: I,
    data: &'a M,
}

impl<'a, I, M> Iter<'a, I, M> {
    pub(crate) fn new(iter: I, data: &'a M) -> Self {
        Self { iter, data }
    }
}

impl<'a, I, M> Iterator for Iter<'a, I, M>
where
    M: Map,
    I: Iterator<Item = &'a M::Key>,
{
    type Item = &'a M::Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let key = self.iter.next()?;
        self.data.get(key)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, I, M> DoubleEndedIterator for Iter<'a, I, M>
where
    M: Map,
    I: DoubleEndedIterator<Item = &'a M::Key>,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let key = self.iter.next_back()?;
        self.data.get(key)
    }
}

impl<'a, I, M> ExactSizeIterator for Iter<'a, I, M>
where
    M: Map,
    I: ExactSizeIterator<Item = &'a M::Key>,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<I, M> Clone for Iter<'_, I, M>
where
    I: Clone,
    M: Map,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            data: self.data,
        }
    }
}
