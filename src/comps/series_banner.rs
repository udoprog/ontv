use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Navigate(Page),
}

#[derive(Default, Debug, Clone)]
pub(crate) struct SeriesBanner;

impl SeriesBanner {
    /// Prepare assets needed for banner.
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, series_id: &SeriesId) {
        if let Some(series) = cx.service.series(series_id) {
            cx.assets.mark_with_hint(series.banner(), BANNER);
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

    /// Generate buttons which perform actions on the given series.
    pub(crate) fn view(&self, cx: &CtxtRef<'_>, series: &Series) -> Element<'static, Message> {
        let handle = match series
            .banner()
            .and_then(|i| cx.assets.image_with_hint(&i, BANNER))
        {
            Some(handle) => handle,
            None => cx.assets.missing_banner(),
        };

        let banner = w::image(handle);

        let title = link(w::text(&series.title).size(TITLE_SIZE))
            .on_press(Message::Navigate(page::series::page(series.id)));

        w::Column::new()
            .push(banner)
            .push(title)
            .spacing(GAP)
            .width(Length::Fill)
            .align_items(Alignment::Center)
            .into()
    }
}
