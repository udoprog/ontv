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
    fn refresh(&mut self, ctx: &Context<Self>) {
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
                self.refresh(ctx);
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

        this.refresh(&ctx);
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
            <div class="container">
                <h1>{"Dashboard"}</h1>
                {for dashboard.days.iter().map(|day| {
                    html! {
                        <div class="day">
                            <h2 class="day-title">
                                {format!("{}", day.date)}
                            </h2>

                            {for day.series.iter().map(|s| {
                                html! {
                                    <div class="series">
                                    <div class="series-title">
                                        {s.title}
                                    </div>

                                    <div class="series-content">
                                    {for s.poster.as_ref().map(|poster| {
                                        html! {
                                            <img src={format!("/api/image/fill-240x360/{poster}")} width="240" />
                                        }
                                    })}

                                    <div class="series-episodes">
                                    {for s.episodes.iter().map(|e| {
                                        html! {
                                            <div>{format!("{}x{} {}", e.season.short(), e.number, e.name.unwrap_or("TBA"))}</div>
                                        }
                                    })}
                                    </div>
                                    </div>
                                    </div>
                                }
                            })}
                        </div>
                    }
                })}
            </div>
        }
    }
}
