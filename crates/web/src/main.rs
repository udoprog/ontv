mod components;
mod error;

use musli_yew::ws;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
enum Route {
    #[at("/")]
    Dashboard,
    #[not_found]
    #[at("/404")]
    NotFound,
}

struct App {
    ws: ws::Service<Self>,
    handle: ws::Handle,
}

enum Msg {
    WebSocket(ws::Msg),
    Error(error::Error),
}

impl From<ws::Msg> for Msg {
    #[inline]
    fn from(value: ws::Msg) -> Self {
        Self::WebSocket(value)
    }
}

impl From<error::Error> for Msg {
    #[inline]
    fn from(error: error::Error) -> Self {
        Self::Error(error)
    }
}

impl From<musli_yew::ws::Error> for Msg {
    #[inline]
    fn from(error: musli_yew::ws::Error) -> Self {
        Self::Error(error::Error::from(error))
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

    fn view(&self, _: &Context<Self>) -> Html {
        let ws = self.handle.clone();

        html! {
            <BrowserRouter>
                <Switch<Route> render={move |route| switch(route, &ws)} />
            </BrowserRouter>
        }
    }
}

fn switch(routes: Route, ws: &ws::Handle) -> Html {
    match routes {
        Route::Dashboard => html!(<components::Dashboard ws={ws} />),
        Route::NotFound => {
            html! {
                <div id="content" class="container">{"There is nothing here"}</div>
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    wasm_logger::init(wasm_logger::Config::default());
    log::trace!("Started up");
    yew::Renderer::<App>::new().render();
    Ok(())
}
