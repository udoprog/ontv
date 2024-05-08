use api::SeriesId;
use derive_more::From;
use musli_yew::ws;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{Dashboard, Series};
use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub(crate) enum Route {
    #[at("/")]
    Dashboard,
    #[at("/series/:id")]
    Series { id: SeriesId },
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub(crate) struct App {
    ws: ws::Service<Self>,
    handle: ws::Handle,
}

#[derive(From)]
pub(crate) enum Msg {
    WebSocket(ws::Msg),
    Error(Error),
}

impl From<musli_yew::ws::Error> for Msg {
    #[inline]
    fn from(error: musli_yew::ws::Error) -> Self {
        Self::from(Error::from(error))
    }
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (ws, handle) = ws::Service::new(ctx);
        let mut this = Self { ws, handle };
        this.ws.connect(ctx);
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::WebSocket(msg) => {
                self.ws.update(ctx, msg);
                false
            }
            Msg::Error(error) => {
                log::error!("Failed to fetch: {error}");
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let ws = self.handle.clone();

        let onerror = ctx.link().callback(|error| error);

        html! {
            <BrowserRouter>
                <Switch<Route> render={move |route| switch(route, &ws, &onerror)} />
            </BrowserRouter>
        }
    }
}

fn switch(routes: Route, ws: &ws::Handle, onerror: &Callback<Error>) -> Html {
    match routes {
        Route::Dashboard => html!(<Dashboard {ws} {onerror} />),
        Route::Series { id } => html!(<Series {ws} {onerror} {id} />),
        Route::NotFound => {
            html! {
                <div id="content" class="container">{"There is nothing here"}</div>
            }
        }
    }
}
