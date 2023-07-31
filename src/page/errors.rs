use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {}

#[derive(Default)]
pub(crate) struct Errors;

impl Errors {
    pub(crate) fn view(&self, cx: &CtxtRef<'_>) -> Element<'static, Message> {
        let mut page = w::Column::new();

        for e in cx.state.errors().rev() {
            let mut error = w::Column::new();

            match e.id {
                Some(ErrorId::Search(..)) => {
                    error = error.push(
                        w::text("Search error")
                            .size(SUBTITLE_SIZE)
                            .style(cx.warning_text()),
                    );
                }
                None => {
                    error = error.push(
                        w::text("Error")
                            .size(SUBTITLE_SIZE)
                            .style(cx.warning_text()),
                    );
                }
            }

            error = error.push(w::text(format!("At: {}", e.timestamp)).size(SMALL_SIZE));
            error = error.push(w::text(&e.message));

            for cause in &e.causes {
                error = error.push(w::text(format!("Caused by: {cause}")));
            }

            page = page.push(error.spacing(SPACE));
        }

        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}
