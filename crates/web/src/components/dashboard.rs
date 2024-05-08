use std::collections::HashMap;

use api::{Config, Episode, EpisodeId, ImageHint, ScheduledDay, Series, SeriesId};
use derive_more::From;
use musli_yew::ws;
use musli_yew::ws::Packet;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::Route;
use crate::error::Error;

pub(crate) struct Dashboard {
    config: Option<Config>,
    series: HashMap<SeriesId, Series>,
    episodes: HashMap<EpisodeId, Episode>,
    schedule: Vec<ScheduledDay>,
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
        Self::from(Error::from(error))
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) ws: ws::Handle,
    pub(crate) onerror: Callback<Error>,
}

impl Component for Dashboard {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            config: None,
            series: HashMap::new(),
            episodes: HashMap::new(),
            schedule: Vec::new(),
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
                    self.series.clear();
                    self.episodes.clear();

                    self.schedule = update.schedule;

                    for s in update.series {
                        self.series.insert(s.id, s);
                    }

                    for e in update.episodes {
                        self.episodes.insert(e.id, e);
                    }
                });

                true
            }
            Msg::Error(error) => {
                ctx.props().onerror.emit(error);
                false
            }
        }
    }

    fn view(&self, _: &Context<Self>) -> Html {
        html! {
            <div>
                <h1>{"Dashboard"}</h1>
                {for self.schedule.iter().map(|day| {
                    html! {
                        <div>
                            <h2>{day.date.to_string()}</h2>
                            <div>
                                {for day.schedule.iter().flat_map(|entry| {
                                    let series = self.series.get(&entry.series_id)?;

                                    let poster = series.graphics.poster.as_ref()?;

                                    Some(html! {
                                        <div>
                                            <h3>{series.title.clone()}</h3>

                                            <Link<Route> to={Route::Series { id: series.id }}>
                                                <img src={poster.url(ImageHint::Fit(60, 120))} alt={series.title.clone()} />
                                            </Link<Route>>

                                            <ul>
                                                {for entry.episodes.iter().flat_map(|id| {
                                                    let e = self.episodes.get(id)?;
                                                    let name = e.name.as_ref()?;

                                                    Some(html! {
                                                        <li>{name.clone()}</li>
                                                    })
                                                })}
                                            </ul>
                                        </div>
                                    })
                                })}
                            </div>
                        </div>
                    }
                })}
            </div>
        }
    }
}
