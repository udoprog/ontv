use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Navigate(Page),
}

#[derive(Default, Debug, Clone)]
pub(crate) struct MovieBanner;

impl MovieBanner {
    /// Prepare assets needed for banner.
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, movie_id: &MovieId) {
        if let Some(movie) = cx.service.movie(movie_id) {
            cx.assets.mark_with_hint(movie.banner(), BANNER);
        }
    }

    /// Update message.
    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::Navigate(page) => {
                cx.push_history(page);
            }
        }
    }

    /// Generate buttons which perform actions on the given movie.
    pub(crate) fn view(&self, cx: &CtxtRef<'_>, movie: &Movie) -> Element<'static, Message> {
        let handle = match movie
            .banner()
            .and_then(|i| cx.assets.image_with_hint(i, BANNER))
        {
            Some(handle) => handle,
            None => cx.assets.missing_banner(),
        };

        let banner = w::image(handle);

        let title = link(
            w::text(&movie.title)
                .shaping(w::text::Shaping::Advanced)
                .size(TITLE_SIZE),
        )
        .on_press(Message::Navigate(page::movie::page(movie.id)));

        w::Column::new()
            .push(banner)
            .push(title)
            .spacing(GAP)
            .width(Length::Fill)
            .align_items(Alignment::Center)
            .into()
    }
}
