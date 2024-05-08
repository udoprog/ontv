use api::config::Config;
use derive_more::From;
use musli_yew::ws::Packet;
use yew::prelude::*;

use crate::error::Error;
use crate::ws;

pub(crate) struct Dashboard {
    config: Option<Config>,
    _updates: ws::Listener<api::broadcast::DashboardUpdateBroadcast>,
    _initialize: ws::Request<api::request::InitializeDashboard>,
}

#[derive(From)]
pub(crate) enum Msg {
    DashboardUpdate(Packet<api::broadcast::DashboardUpdateBroadcast>),
    InitializeDashboard(Packet<api::request::InitializeDashboard>),
    Error(Error),
}

impl From<musli_yew::ws::Error> for Msg {
    #[inline]
    fn from(error: musli_yew::ws::Error) -> Self {
        Self::Error(Error::from(error))
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) ws: ws::Handle,
}

impl Component for Dashboard {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            config: None,
            _updates: ctx.props().ws.listen(ctx),
            _initialize: ctx
                .props()
                .ws
                .request(ctx, api::request::InitializeDashboardRequest),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DashboardUpdate(packet) => {
                packet.decode(ctx, |update| {
                    self.config = Some(update.config);
                });

                true
            }
            Msg::InitializeDashboard(packet) => {
                packet.decode(ctx, |update| {
                    self.config = Some(update.config);
                });

                true
            }
            Msg::Error(error) => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div>
                <h1>{"Dashboard"}</h1>
            </div>
        }
    }
}
