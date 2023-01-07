/// Helper option iterator.
pub(crate) struct OptionIter<I> {
    iter: Option<I>,
}

impl<I> OptionIter<I> {
    /// Construct a new option iterator.
    #[inline]
    pub(crate) fn new(iter: Option<I>) -> Self {
        Self { iter }
    }
}

impl<I> Iterator for OptionIter<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut iter = self.iter.take()?;
        let item = iter.next()?;
        self.iter = Some(iter);
        Some(item)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let mut iter = self.iter.take()?;
        let item = iter.nth(n)?;
        self.iter = Some(iter);
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.iter {
            Some(iter) => iter.size_hint(),
            None => (0, Some(0)),
        }
    }
}

impl<I> DoubleEndedIterator for OptionIter<I>
where
    I: DoubleEndedIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let mut iter = self.iter.take()?;
        let item = iter.next_back()?;
        self.iter = Some(iter);
        Some(item)
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let mut iter = self.iter.take()?;
        let item = iter.nth_back(n)?;
        self.iter = Some(iter);
        Some(item)
    }
}

impl<I> ExactSizeIterator for OptionIter<I> where I: ExactSizeIterator {}
