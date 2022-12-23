/// Helper to deal with components.
pub(crate) trait Component<T> {
    fn new(params: T) -> Self;

    /// reinitialize component.
    fn changed(&mut self, params: T);
}

pub(crate) trait ComponentInitExt<C> {
    /// Initialize components from an iterator, will ensure that the length of
    /// the initialized component matches the data and avoid re-allocating when
    /// possible.
    fn init_from_iter<I, T>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        C: Component<T>;
}

impl<C> ComponentInitExt<C> for Vec<C> {
    fn init_from_iter<I, T>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        C: Component<T>,
    {
        let mut it = iter.into_iter();
        let mut len = 0;
        let mut this = self.iter_mut();

        while let Some((out, data)) = this.next().and_then(|out| Some((out, it.next()?))) {
            out.changed(data);
            len += 1;
        }

        for data in it {
            self.push(C::new(data));
            len += 1;
        }

        self.truncate(len);
    }
}

impl<C> ComponentInitExt<C> for Option<C> {
    fn init_from_iter<I, T>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        C: Component<T>,
    {
        let mut it = iter.into_iter();

        match (self.as_mut(), it.next()) {
            (Some(c), Some(value)) => c.changed(value),
            (None, Some(value)) => {
                *self = Some(C::new(value));
            }
            (Some(..), None) => {
                *self = None;
            }
            (None, None) => {}
        }
    }
}
