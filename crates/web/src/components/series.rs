use api::SeriesId;
use derive_more::From;
use musli_yew::ws;
use yew::prelude::*;

use crate::error::Error;

pub(crate) struct Series {
    _updates: ws::Listener<api::broadcast::SeriesUpdateBroadcast>,
}

#[derive(From)]
pub(crate) enum Msg {
    SeriesUpdate(ws::Packet<api::broadcast::SeriesUpdateBroadcast>),
    Error(Error),
}

impl From<musli_yew::ws::Error> for Msg {
    #[inline]
    fn from(error: musli_yew::ws::Error) -> Self {
        Self::from(Error::from(error))
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) ws: ws::Handle,
    pub(crate) onerror: Callback<Error>,
    pub(crate) id: SeriesId,
}

impl Component for Series {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            _updates: ctx.props().ws.listen(ctx),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SeriesUpdate(packet) => {
                packet.decode(ctx, |_| {});
                true
            }
            Msg::Error(error) => {
                ctx.props().onerror.emit(error);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div>
                <h1>{format!("Series {}", ctx.props().id)}</h1>
            </div>
        }
    }
}
