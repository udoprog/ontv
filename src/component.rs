/// Helper to deal with components.
pub(crate) trait Component<T> {
    fn new(params: T) -> Self;

    /// reinitialize component.
    fn init(&mut self, params: T);
}

pub(crate) trait ComponentExt<C> {
    /// Initialize components from an iterator.
    fn initialize_iter<I, T>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        C: Component<T>;
}

impl<C> ComponentExt<C> for Vec<C> {
    fn initialize_iter<I, T>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        C: Component<T>,
    {
        let mut it = iter.into_iter();
        let mut len = 0;
        let mut this = self.iter_mut();

        if let Some((out, data)) = this.next().and_then(|out| Some((out, it.next()?))) {
            out.init(data);
            len += 1;
        }

        for data in it {
            self.push(C::new(data));
            len += 1;
        }

        self.truncate(len);
    }
}
