use derive_more::From;
use musli_web::web::Packet;
use yew::prelude::*;

use crate::error::Error;
use crate::ws;

pub(crate) struct Dashboard {
    _initialize: ws::Request,
    _state_change: ws::StateListener,
    state: ws::State,
    dashboard: Packet<api::InitializeDashboard>,
}

#[derive(From)]
pub(crate) enum Msg {
    InitializeDashboard(Result<Packet<api::InitializeDashboard>, ws::Error>),
    StateChanged(ws::State),
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) ws: ws::Handle,
}

impl Dashboard {
    fn on_state_change(&mut self, ctx: &Context<Self>) {
        if matches!(self.state, ws::State::Open) {
            self._initialize = ctx
                .props()
                .ws
                .request()
                .body(api::RequestDashboard)
                .on_packet(ctx.link().callback(Msg::InitializeDashboard))
                .send();
        }
    }

    fn update_fallible(&mut self, ctx: &Context<Self>, msg: Msg) -> Result<bool, Error> {
        match msg {
            Msg::InitializeDashboard(result) => {
                log::info!("Dashboard initialized");
                self.dashboard = result?;
                Ok(true)
            }
            Msg::StateChanged(state) => {
                self.state = state;
                self.on_state_change(ctx);
                Ok(false)
            }
        }
    }
}

impl Component for Dashboard {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let (state, _state_change) = ctx
            .props()
            .ws
            .on_state_change(ctx.link().callback(Msg::StateChanged));

        let mut this = Self {
            _initialize: ws::Request::new(),
            _state_change,
            state,
            dashboard: Packet::empty(),
        };

        this.on_state_change(&ctx);
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match self.update_fallible(ctx, msg) {
            Ok(changed) => changed,
            Err(error) => {
                log::error!("Dashboard error: {error}");
                false
            }
        }
    }

    fn view(&self, _: &Context<Self>) -> Html {
        let Ok(dashboard) = self.dashboard.decode() else {
            return html! {
                <div>
                    <h1>{"Dashboard"}</h1>
                    <p>{"Loading dashboard..."}</p>
                </div>
            };
        };

        html! {
            <div>
                <h1>{"Dashboard"}</h1>
                {for dashboard.days.iter().map(|day| {
                    html! {
                        <div>
                            {format!("{}", day.date)}

                            <ul>
                            {for day.series.iter().map(|s| {
                                html! {
                                    <li>
                                        {format!("Series: {}", s.title)}

                                        <ul>
                                        {for s.episodes.iter().map(|e| {
                                            html! {
                                                <li>{format!("{}x{} {}", e.season.short(), e.number, e.name.unwrap_or("TBA"))}</li>
                                            }
                                        })}
                                        </ul>
                                    </li>
                                }
                            })}
                            </ul>
                        </div>
                    }
                })}
            </div>
        }
    }
}
