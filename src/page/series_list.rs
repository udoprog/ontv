use crate::prelude::*;

/// Messages generated and handled by [SeriesList].
#[derive(Debug, Clone)]
pub(crate) enum Message {
    ChangeFilter(String),
    SeriesActions(usize, comps::series_actions::Message),
    Navigate(Page),
}

#[derive(Default)]
pub(crate) struct SeriesList {
    filter: String,
    filtered: Option<Box<[SeriesId]>>,
    actions: Vec<comps::SeriesActions>,
}

impl SeriesList {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>) {
        if let Some(filtered) = &self.filtered {
            let series = filtered.iter().flat_map(|id| cx.service.series(id));
            self.actions.init_from_iter(series.clone().map(|s| s.id));
            cx.assets
                .mark_with_hint(series.flat_map(|s| s.poster()), POSTER_HINT);
        } else {
            self.actions
                .init_from_iter(cx.service.series_by_name().map(|s| s.id));

            cx.assets.mark_with_hint(
                cx.service.series_by_name().flat_map(|s| s.poster()),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::ChangeFilter(filter) => {
                self.filter = filter;
                let filter = crate::search::Tokens::new(&self.filter);

                self.filtered = if !filter.is_empty() {
                    let mut filtered = Vec::new();

                    for s in cx.service.series_by_name() {
                        if filter.matches(&s.title) {
                            filtered.push(s.id);
                        }
                    }

                    Some(filtered.into())
                } else {
                    None
                };
            }
            Message::SeriesActions(index, message) => {
                if let Some(actions) = self.actions.get_mut(index) {
                    actions.update(cx, message);
                }
            }
            Message::Navigate(page) => {
                cx.push_history(page);
            }
        }
    }

    pub(crate) fn view(&self, cx: &CtxtRef<'_>) -> Element<'static, Message> {
        let mut rows = w::Column::new();

        let mut it;
        let mut it2;

        let iter: &mut dyn Iterator<Item = _> = if let Some(filtered) = &self.filtered {
            it = filtered.iter().flat_map(|id| cx.service.series(id));
            &mut it
        } else {
            it2 = cx.service.series_by_name();
            &mut it2
        };

        for (index, (series, actions)) in iter.zip(&self.actions).enumerate() {
            let poster = match series
                .poster()
                .and_then(|i| cx.assets.image_with_hint(i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => cx.missing_poster(),
            };

            let graphic = link(w::image(poster).height(IMAGE_HEIGHT))
                .on_press(Message::Navigate(page::series::page(series.id)));

            let episodes = cx.service.episodes(&series.id);

            let title = link(
                w::text(&series.title)
                    .shaping(w::text::Shaping::Advanced)
                    .size(SUBTITLE_SIZE),
            )
            .on_press(Message::Navigate(page::series::page(series.id)));

            let actions = actions
                .view(cx, series)
                .map(move |m| Message::SeriesActions(index, m));

            let mut content = w::Column::new().width(Length::Fill);

            content = content.push(
                w::Column::new()
                    .push(title)
                    .push(w::text(format!("{} episode(s)", episodes.len())))
                    .push(actions)
                    .spacing(SPACE),
            );

            if !series.overview.is_empty() {
                content =
                    content.push(w::text(&series.overview).shaping(w::text::Shaping::Advanced));
            }

            rows = rows.push(
                centered(
                    w::Row::new()
                        .push(graphic)
                        .push(content.spacing(GAP))
                        .spacing(GAP)
                        .width(Length::Fill),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let filter = w::text_input("Filter...", &self.filter)
            .on_input(Message::ChangeFilter)
            .width(Length::Fill);

        w::Column::new()
            .push(centered(
                w::Row::new().push(filter).padding(GAP).width(Length::Fill),
                None,
            ))
            .push(rows.spacing(GAP2))
            .width(Length::Fill)
            .into()
    }
}
