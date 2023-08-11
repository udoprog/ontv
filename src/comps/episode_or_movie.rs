use crate::component::Component;
use crate::comps;
use crate::model::Watched;
use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Episode(comps::episode::Message),
    Movie(comps::movie_item::Message),
}

#[derive(PartialEq, Eq)]
pub(crate) enum Props<A, B> {
    Episode(comps::episode::Props<A>),
    Movie(comps::movie_item::Props<B>),
}

pub(crate) enum EpisodeOrMovie {
    Episode(comps::Episode),
    Movie(comps::MovieItem),
}

impl<'a, A, B> Component<Props<A, B>> for EpisodeOrMovie
where
    A: DoubleEndedIterator<Item = &'a Watched> + Clone,
    B: DoubleEndedIterator<Item = &'a Watched> + Clone,
{
    #[inline]
    fn new(props: Props<A, B>) -> Self {
        match props {
            Props::Episode(props) => Self::Episode(comps::Episode::new(props)),
            Props::Movie(props) => Self::Movie(comps::MovieItem::new(props)),
        }
    }

    #[inline]
    fn changed(&mut self, props: Props<A, B>) {
        let props = match (&mut *self, props) {
            (Self::Episode(e), Props::Episode(props)) => return e.changed(props),
            (Self::Movie(m), Props::Movie(props)) => return m.changed(props),
            (_, props) => props,
        };

        *self = <Self as Component<Props<A, B>>>::new(props)
    }
}

impl EpisodeOrMovie {
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>) {
        match self {
            EpisodeOrMovie::Episode(episode) => episode.prepare(cx),
            EpisodeOrMovie::Movie(movie) => movie.prepare(cx),
        }
    }

    pub(crate) fn view(
        &self,
        cx: &CtxtRef<'_>,
        pending: bool,
    ) -> Result<Element<'static, Message>> {
        match self {
            EpisodeOrMovie::Episode(episode) => {
                Ok(episode.view(cx, pending)?.map(Message::Episode))
            }
            EpisodeOrMovie::Movie(movie) => Ok(movie.view(cx, pending)?.map(Message::Movie)),
        }
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, m: Message) {
        match (self, m) {
            (Self::Episode(episode), Message::Episode(m)) => episode.update(cx, m),
            (Self::Movie(movie), Message::Movie(m)) => movie.update(cx, m),
            _ => {}
        }
    }
}
