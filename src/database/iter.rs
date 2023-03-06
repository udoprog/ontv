use std::collections::HashMap;
use std::hash::Hash;

pub(crate) struct Iter<'a, I, K, V> {
    iter: I,
    data: &'a HashMap<K, V>,
}

impl<'a, I, K, V> Iter<'a, I, K, V> {
    pub(crate) fn new(iter: I, data: &'a HashMap<K, V>) -> Self {
        Self { iter, data }
    }
}

impl<'a, I, K, V> Iterator for Iter<'a, I, K, V>
where
    K: Eq + Hash,
    I: Iterator<Item = &'a K>,
{
    type Item = &'a V;

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

impl<'a, I, K, V> DoubleEndedIterator for Iter<'a, I, K, V>
where
    K: Eq + Hash,
    I: DoubleEndedIterator<Item = &'a K>,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let key = self.iter.next_back()?;
        self.data.get(key)
    }
}

impl<'a, I, K, V> ExactSizeIterator for Iter<'a, I, K, V>
where
    K: Eq + Hash,
    I: ExactSizeIterator<Item = &'a K>,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, I, K, V> Clone for Iter<'a, I, K, V>
where
    I: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            data: self.data,
        }
    }
}
