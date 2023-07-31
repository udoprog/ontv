use std::time::Duration;

use iced::advanced::image::Handle;
use iced::window;
use iced::{Command, Theme};

use crate::assets::{Assets, ImageKey};
use crate::commands::{Commands, CommandsBuf};
use crate::context::{Ctxt, CtxtRef};
use crate::database::SeasonRef;
use crate::error::ErrorInfo;
use crate::history::{History, HistoryMutations, Page};
use crate::model::ImageV2;
use crate::page;
use crate::params::{GAP, SMALL_SIZE, SPACE, SUB_MENU_SIZE};
use crate::prelude::*;
use crate::queue::{Task, TaskKind};
use crate::service::{NewSeries, Service};
use crate::state::State;
use crate::utils::{Singleton, TimedOut, Timeout};

macro_rules! ctxt {
    ($self:expr) => {
        &mut Ctxt {
            state: &mut $self.state,
            history: &mut $self.history_mutations,
            service: &mut $self.service,
            assets: &mut $self.assets,
        }
    };
}

macro_rules! ctxt_ref {
    ($self:expr) => {
        &CtxtRef {
            state: &$self.state,
            service: &$self.service,
            assets: &$self.assets,
        }
    };
}

// Check for remote updates every two minutes.
const UPDATE_TIMEOUT: u64 = 120;
// Number of images to process in parallel.
const IMAGE_BATCH: usize = 10;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Platform-specific events.
    CloseRequested,
    Settings(page::settings::Message),
    Dashboard(page::dashboard::Message),
    WatchNext(page::watch_next::Message),
    Search(page::search::Message),
    SeriesList(page::series_list::Message),
    Series(page::series::Message),
    Movie(page::movie::Message),
    Season(page::season::Message),
    Queue(page::queue::Message),
    Errors(page::errors::Message),
    /// Save application changes.
    Save(TimedOut),
    /// Application state was saved.
    Saved(Result<(), ErrorInfo>),
    /// Check for updates.
    CheckForUpdates(TimedOut),
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Navigate history by the specified stride.
    History(isize),
    /// A scroll happened.
    Scroll(w::scrollable::Viewport),
    /// Images have been loaded in the background.
    ImagesLoaded(Result<Vec<(ImageKey, Handle)>, ErrorInfo>),
    /// Update download queue with the given items.
    TaskUpdateDownloadQueue(Result<Option<TaskKind>, ErrorInfo>, Task),
    /// Task output of add series by remote.
    TaskSeriesDownloaded(Result<Option<NewSeries>, ErrorInfo>, Task),
    /// Queue processing.
    ProcessQueue(TimedOut, TaskId),
}

/// Current page state.
enum Current {
    Dashboard(page::Dashboard),
    WatchNext(page::WatchNext),
    Settings(page::Settings),
    Search(page::Search),
    Series(page::Series),
    Movie(page::Movie),
    SeriesList(page::SeriesList),
    Season(page::Season),
    Queue(page::Queue),
    Errors(page::Errors),
}

/// Main application.
pub(crate) struct Application {
    /// Our own command buffer.
    commands: CommandsBuf<Message>,
    /// Application state.
    state: State,
    /// Application history.
    history: History,
    /// History mutations.
    history_mutations: HistoryMutations,
    /// Backing service.
    service: Service,
    /// Assets manager.
    assets: Assets,
    /// Current page state.
    current: Current,
    // Timeout before database changes are saved to the filesystem.
    database_timeout: Timeout,
    // Timeout to populate the update queue.
    update_timeout: Timeout,
    // Timeout until the next queue should wakeup.
    queue_timeout: Timeout,
    /// Image loader future being run.
    image_loader: Singleton,
    // Exit after save has been completed.
    exit_after_save: bool,
    // Images to load.
    images: Vec<(ImageKey, ImageV2)>,
    /// The identifier used for the main scrollable.
    scrollable_id: w::scrollable::Id,
}

pub(crate) struct Flags {
    pub(crate) service: Service,
}

impl iced::Application for Application {
    type Executor = iced_futures::backend::native::tokio::Executor;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let today = Utc::now().date_naive();

        let state = State::new(today);
        let current = Current::Dashboard(page::dashboard::Dashboard::new(&state, &flags.service));

        let mut this = Application {
            commands: CommandsBuf::default(),
            state,
            history: History::new(),
            history_mutations: HistoryMutations::default(),
            service: flags.service,
            assets: Assets::new(),
            current,
            database_timeout: Timeout::default(),
            update_timeout: Timeout::default(),
            queue_timeout: Timeout::default(),
            image_loader: Singleton::default(),
            exit_after_save: false,
            images: Vec::new(),
            scrollable_id: w::scrollable::Id::unique(),
        };

        this.prepare();
        this.handle_image_loading();
        this.handle_process_queue(None);
        this.commands
            .perform(async { TimedOut::TimedOut }, Message::CheckForUpdates);
        let command = this.commands.build();
        (this, command)
    }

    #[inline]
    fn title(&self) -> String {
        const BASE: &str = "OnTV";

        if let Some(page) = self.history.page() {
            match page {
                Page::Dashboard => {
                    return format!("{BASE} - Dashboard");
                }
                Page::WatchNext(..) => return format!("{BASE} - Watch next"),
                Page::Search(..) => {
                    return format!("{BASE} - Search");
                }
                Page::SeriesList => {
                    return format!("{BASE} - Series overview");
                }
                Page::Series(state) => {
                    if let Some(series) = self.service.series(&state.id) {
                        return format!("{BASE} - {}", series.title);
                    }
                }
                Page::Movie(state) => {
                    if let Some(movie) = self.service.movie(&state.id) {
                        return format!("{BASE} - {}", movie.title);
                    }
                }
                Page::Settings => {
                    return format!("{BASE} - Settings");
                }
                Page::Season(state) => {
                    if let Some(series) = self.service.series(&state.series_id) {
                        return format!(
                            "{BASE} - {} - {season}",
                            series.title,
                            season = state.season
                        );
                    }
                }
                Page::Queue(..) => {
                    return format!("{BASE} - Queue");
                }
                Page::Errors => {
                    let errors = self.state.errors().len();
                    return format!("{BASE} - Errors ({errors})");
                }
            }
        }

        BASE.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        tracing::trace!("{message:?}");

        match (message, &mut self.current, self.history.page_mut()) {
            (Message::Settings(message), Current::Settings(page), _) => {
                page.update(ctxt!(self), message);
            }
            (Message::Dashboard(message), Current::Dashboard(page), _) => {
                page.update(ctxt!(self), message);
            }
            (
                Message::WatchNext(message),
                Current::WatchNext(page),
                Some(Page::WatchNext(state)),
            ) => {
                page.update(ctxt!(self), state, message);
            }
            (Message::Search(message), Current::Search(page), Some(Page::Search(state))) => {
                page.update(
                    ctxt!(self),
                    state,
                    message,
                    self.commands.by_ref().map(Message::Search),
                );
            }
            (Message::SeriesList(message), Current::SeriesList(page), _) => {
                page.update(ctxt!(self), message);
            }
            (Message::Series(message), Current::Series(page), _) => {
                page.update(ctxt!(self), message);
            }
            (Message::Season(message), Current::Season(page), _) => {
                page.update(ctxt!(self), message);
            }
            (Message::Queue(message), Current::Queue(page), _) => {
                page.update(
                    ctxt!(self),
                    message,
                    self.commands.by_ref().map(Message::Queue),
                );
            }
            (Message::CloseRequested, _, _) => {
                tracing::debug!("Close requested");

                self.exit_after_save = true;

                if self.database_timeout.is_set() {
                    self.database_timeout.clear();
                } else {
                    self.commands.command(window::close());
                }

                return self.commands.build();
            }
            (Message::Save(timed_out), _, _) => {
                // To avoid a cancellation loop we need to return here.
                if !matches!(timed_out, TimedOut::TimedOut) && !self.exit_after_save {
                    return self.commands.build();
                }

                self.database_timeout.clear();
                self.state.set_saving(true);

                self.commands
                    .perform(self.service.save_changes(), |result| match result {
                        Ok(()) => Message::Saved(Ok(())),
                        Err(error) => Message::Saved(Err(error.into())),
                    })
            }
            (Message::Saved(result), _, _) => {
                if let Err(error) = result {
                    self.state.handle_error(error);
                }

                if self.exit_after_save {
                    self.commands.command(window::close());
                }

                self.state.set_saving(false);
            }
            (Message::CheckForUpdates(TimedOut::TimedOut), _, _) => {
                let now = Utc::now();
                self.service.find_updates(&now);
                let today = now.date_naive();

                if *self.state.today() != today {
                    self.state.set_today(today);
                }

                // Schedule next update.
                self.commands.perform(
                    self.update_timeout.set(Duration::from_secs(UPDATE_TIMEOUT)),
                    Message::CheckForUpdates,
                );
            }
            (Message::TaskUpdateDownloadQueue(result, task), _, _) => {
                let now = Utc::now();
                self.service.complete_task(&now, task);

                match result {
                    Ok(task) => {
                        if let Some(task) = task {
                            self.service.push_task(&now, task);
                        }
                    }
                    Err(error) => {
                        self.state.handle_error(error);
                    }
                }
            }
            (Message::Navigate(page), _, _) => {
                self.history_mutations.push_history(&mut self.assets, page);
            }
            (Message::History(relative), _, _) => {
                self.history_mutations.navigate(relative);
            }
            (Message::Scroll(offset), _, _) => {
                self.history.history_scroll(offset.relative_offset());
            }
            (Message::ImagesLoaded(loaded), _, _) => {
                match loaded {
                    Ok(loaded) => {
                        self.assets.insert_images(loaded);
                    }
                    Err(error) => {
                        tracing::error!("error loading images: {error}");
                    }
                }

                self.image_loader.clear();
                self.handle_image_loading();
                return self.commands.build();
            }
            (Message::TaskSeriesDownloaded(result, task), _, _) => {
                let now = Utc::now();
                self.service.complete_task(&now, task);

                match result {
                    Ok(new_series) => {
                        if let Some(new_series) = new_series {
                            self.service.insert_series(&now, new_series);
                        }
                    }
                    Err(error) => {
                        self.state.handle_error(error);
                    }
                }
            }
            (Message::ProcessQueue(TimedOut::TimedOut, id), _, _) => {
                self.handle_process_queue(Some(id));
            }
            _ => {}
        };

        if self.service.has_changes() && !self.exit_after_save {
            self.commands.perform(
                self.database_timeout.set(Duration::from_secs(5)),
                Message::Save,
            );
        }

        if let Some((page, scroll)) = self.history.apply_mutation(&mut self.history_mutations) {
            self.current = match page {
                Page::Dashboard => {
                    Current::Dashboard(page::Dashboard::new(&self.state, &self.service))
                }
                Page::WatchNext(..) => Current::WatchNext(page::WatchNext::default()),
                Page::Search(..) => Current::Search(page::Search::default()),
                Page::SeriesList => Current::SeriesList(page::SeriesList::default()),
                Page::Series(state) => Current::Series(page::Series::new(state)),
                Page::Movie(state) => Current::Movie(page::Movie::new(state)),
                Page::Settings => Current::Settings(page::Settings::default()),
                Page::Season(state) => Current::Season(page::Season::new(state)),
                Page::Queue(..) => {
                    let page = page::Queue::new(self.commands.by_ref().map(Message::Queue));
                    Current::Queue(page)
                }
                Page::Errors => Current::Errors(page::Errors::default()),
            };

            self.commands
                .command(w::scrollable::snap_to(self.scrollable_id.clone(), *scroll));
        }

        self.prepare();

        self.handle_image_loading();

        if self.service.take_tasks_modified() {
            self.handle_process_queue(None)
        }

        self.commands.build()
    }

    #[inline]
    fn subscription(&self) -> iced::Subscription<Self::Message> {
        use iced::{event, mouse, Event};
        return iced::subscription::events_with(handle_event);

        fn handle_event(event: Event, status: event::Status) -> Option<Message> {
            let event::Status::Ignored = status else {
                return None;
            };

            tracing::trace!(?event);

            match event {
                Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested),
                Event::Mouse(mouse::Event::ButtonPressed(button)) => match button {
                    mouse::Button::Other(1) => Some(Message::History(-1)),
                    mouse::Button::Other(2) => Some(Message::History(1)),
                    _ => None,
                },
                _ => None,
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let mut top_menu = w::Row::new().spacing(GAP).align_items(Alignment::Center);

        let Some(page) = self.history.page() else {
            return w::text("missing history entry").into();
        };

        top_menu = top_menu.push(menu_item(
            page,
            w::text("Dashboard"),
            |p| matches!(p, Page::Dashboard),
            || Page::Dashboard,
        ));

        top_menu = top_menu.push(menu_item(
            page,
            w::text("Series"),
            |p| matches!(p, Page::SeriesList),
            || Page::SeriesList,
        ));

        top_menu = top_menu.push(menu_item(
            page,
            w::text("Search"),
            |p| matches!(p, Page::Search(..)),
            || Page::Search(page::search::State::default()),
        ));

        top_menu = top_menu.push(menu_item(
            page,
            w::text("Settings"),
            |p| matches!(p, Page::Settings),
            || Page::Settings,
        ));

        // Build queue element.
        {
            let count = self.service.pending_tasks().len() + self.service.running_tasks().len();

            let text = match count {
                0 => w::text("Queue"),
                n => w::text(format!("Queue ({n})")),
            };

            top_menu = top_menu.push(menu_item(
                page,
                text,
                |p| matches!(p, Page::Queue(..)),
                || Page::Queue(page::queue::State::default()),
            ));
        }

        let mut menu = w::Column::new().push(top_menu);

        match page {
            Page::Series(page::series::State { id: series_id }) => {
                let mut sub_menu = w::Row::new();

                if let Some(series) = self.service.series(series_id) {
                    sub_menu = sub_menu.push(render_series(page, series, series_id));
                }

                menu = menu.push(sub_menu.spacing(GAP));
            }
            Page::Season(page::season::State { series_id, season }) => {
                let mut sub_menu = w::Row::new();

                if let Some(series) = self.service.series(series_id) {
                    sub_menu = sub_menu.push(render_series(page, series, series_id));
                }

                let mut seasons = self.service.seasons(series_id);

                if seasons.len() > 5 {
                    if let Some(season) = seasons.next() {
                        sub_menu = sub_menu.push(self.render_season(season, series_id, page));
                    }

                    let last = seasons.next_back();

                    sub_menu = sub_menu.push(w::text("--"));

                    if let Some(season) = seasons.find(|s| s.number == *season) {
                        sub_menu = sub_menu.push(self.render_season(season, series_id, page));
                        sub_menu = sub_menu.push(w::text("--"));
                    }

                    if let Some(season) = last {
                        sub_menu = sub_menu.push(self.render_season(season, series_id, page));
                    }
                } else {
                    for season in seasons {
                        sub_menu = sub_menu.push(self.render_season(season, series_id, page));
                    }
                }

                menu = menu.push(sub_menu.spacing(GAP));
            }
            _ => {}
        }

        let mut window = w::Column::new();

        window = window.push(
            menu.align_items(Alignment::Center)
                .spacing(GAP)
                .padding(GAP),
        );

        let page = match self.render_page() {
            Ok(page) => page,
            Err(e) => {
                return w::text(format_args!("{e}"))
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center)
                    .into();
            }
        };

        window = window.push(w::horizontal_rule(1));
        window = window.push(
            w::scrollable(page)
                .id(self.scrollable_id.clone())
                .on_scroll(Message::Scroll)
                .height(Length::Fill),
        );

        let mut status_bar = w::Row::new();
        let mut any = false;

        if self.state.is_saving() {
            status_bar =
                status_bar.push(w::Row::new().push(w::text("Saving... ").size(SMALL_SIZE)));
            any = true;
        }

        status_bar = status_bar.push(w::Space::new(Length::Fill, Length::Shrink));

        let errors = self.state.errors().len();

        if errors != 0 {
            status_bar = status_bar.push(
                w::button(w::text(format_args!("Errors ({errors})")).size(SMALL_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Navigate(Page::Errors)),
            );
            any = true;
        }

        window = window.push(w::horizontal_rule(1));

        if any {
            window = window.push(
                status_bar
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .align_items(Alignment::Start)
                    .spacing(GAP)
                    .padding(SPACE),
            );
        }

        window
            .align_items(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    #[inline]
    fn theme(&self) -> Theme {
        self.service.theme().clone()
    }
}

fn render_series(
    page: &Page,
    series: &Series,
    series_id: &SeriesId,
) -> w::Button<'static, Message> {
    menu_item(
        page,
        w::text(&series.title)
            .shaping(w::text::Shaping::Advanced)
            .size(SUB_MENU_SIZE),
        |p| matches!(p, Page::Series(page::series::State { id }) if *id == *series_id),
        || page::series::page(*series_id),
    )
}

impl Application {
    // Call prepare on the appropriate components to prepare asset loading.
    fn prepare(&mut self) {
        match (&mut self.current, self.history.page_mut()) {
            (Current::Dashboard(page), _) => {
                page.prepare(ctxt!(self));
            }
            (Current::WatchNext(page), Some(Page::WatchNext(state))) => {
                page.prepare(ctxt!(self), state);
            }
            (Current::Search(page), Some(Page::Search(state))) => {
                page.prepare(
                    ctxt!(self),
                    state,
                    self.commands.by_ref().map(Message::Search),
                );
            }
            (Current::SeriesList(page), _) => {
                page.prepare(ctxt!(self));
            }
            (Current::Series(page), Some(Page::Series(state))) => {
                page.prepare(ctxt!(self), state);
            }
            (Current::Season(page), Some(Page::Season(state))) => {
                page.prepare(ctxt!(self), state);
            }
            _ => {
                // noop
            }
        }

        if self.assets.is_cleared() {
            self.image_loader.clear();
        }

        self.assets.commit();
    }

    /// Handle image loading.
    fn handle_image_loading(&mut self) {
        fn translate(value: Option<Result<Vec<(ImageKey, Handle)>>>) -> Message {
            match value {
                Some(Ok(value)) => Message::ImagesLoaded(Ok(value)),
                None => Message::ImagesLoaded(Ok(Vec::new())),
                Some(Err(e)) => Message::ImagesLoaded(Err(e.into())),
            }
        }

        if self.image_loader.is_set() {
            return;
        }

        self.images.clear();

        while self.images.len() < IMAGE_BATCH {
            let Some((key, image)) = self.assets.next_image() else {
                break;
            };

            self.images.push((key, image));
        }

        if self.images.is_empty() {
            return;
        }

        let future = self.image_loader.set(
            self.service
                .load_images(self.images.drain(..).collect::<Vec<_>>()),
        );
        self.commands.perform(future, translate);
    }

    /// Handle process queue.
    fn handle_process_queue(&mut self, timed_out: Option<TaskId>) {
        let now = Utc::now();

        while let Some(task) = self.service.next_task(&now, timed_out) {
            tracing::trace!("running task {}", task.id);

            match &task.kind {
                TaskKind::CheckForUpdates {
                    series_id,
                    remote_id,
                    last_modified,
                } => {
                    let future =
                        self.service
                            .check_for_updates(*series_id, *remote_id, *last_modified);

                    self.commands.perform(future, move |result| {
                        Message::TaskUpdateDownloadQueue(result.map_err(Into::into), task.clone())
                    });
                }
                TaskKind::DownloadSeries {
                    series_id,
                    remote_id,
                    force,
                    ..
                } => {
                    self.commands.perform(
                        ctxt_ref!(self).download_series_by_id(series_id, remote_id, *force),
                        move |result| {
                            Message::TaskSeriesDownloaded(result.map_err(Into::into), task.clone())
                        },
                    );
                }
                TaskKind::DownloadSeriesByRemoteId { remote_id } => {
                    if self.service.set_series_tracked_by_remote(remote_id) {
                        self.service.complete_task(&now, task);
                    } else {
                        self.commands.perform(
                            self.service.download_series(remote_id, None, None),
                            move |result| {
                                Message::TaskSeriesDownloaded(
                                    result.map_err(Into::into),
                                    task.clone(),
                                )
                            },
                        );
                    }
                }
                TaskKind::DownloadMovieByRemoteId { .. } => {
                    // TODO: implement task
                    self.service.complete_task(&now, task);
                }
            }
        }

        let now = Utc::now();

        if let Some((seconds, id)) = self.service.next_task_sleep(&now) {
            tracing::trace!("next queue sleep: {seconds}s");

            self.commands.perform(
                self.queue_timeout.set(Duration::from_secs(seconds)),
                move |timed_out| Message::ProcessQueue(timed_out, id),
            );
        }
    }

    fn render_page(&self) -> Result<Element<'static, Message>> {
        let page = match (&self.current, self.history.page()) {
            (Current::Dashboard(page), _) => page.view(ctxt_ref!(self)).map(Message::Dashboard),
            (Current::WatchNext(page), Some(Page::WatchNext(state))) => {
                page.view(ctxt_ref!(self), state)?.map(Message::WatchNext)
            }
            (Current::Search(page), Some(Page::Search(state))) => {
                page.view(ctxt_ref!(self), state).map(Message::Search)
            }
            (Current::SeriesList(page), _) => page.view(ctxt_ref!(self)).map(Message::SeriesList),
            (Current::Series(page), Some(Page::Series(series_id))) => {
                page.view(ctxt_ref!(self), series_id)?.map(Message::Series)
            }
            (Current::Movie(page), Some(Page::Movie(state))) => {
                page.view(state).map(Message::Movie)
            }
            (Current::Settings(page), _) => page.view(ctxt_ref!(self)).map(Message::Settings),
            (Current::Season(page), Some(Page::Season(state))) => {
                page.view(ctxt_ref!(self), state)?.map(Message::Season)
            }
            (Current::Queue(page), Some(Page::Queue(state))) => {
                page.view(ctxt_ref!(self), state).map(Message::Queue)
            }
            (Current::Errors(page), _) => page.view(ctxt_ref!(self)).map(Message::Errors),
            _ => return Err(anyhow!("illegal page state")),
        };

        Ok(page)
    }

    fn render_season(
        &self,
        season: SeasonRef<'_>,
        series_id: &SeriesId,
        page: &Page,
    ) -> w::Button<'static, Message> {
        let title = w::text(season.number);
        let (watched, total) = self.service.season_watched(series_id, &season.number);

        let mut title = w::Row::new().push(title.size(SUB_MENU_SIZE));

        if let Some(p) = watched.saturating_mul(100).checked_div(total) {
            title = title.push(w::text(format_args!(" ({p}%)")).size(SUB_MENU_SIZE));
        }

        menu_item(
            page,
            title,
            |p| matches!(p, Page::Season(page::season::State { series_id: a, season: b }) if *a == *series_id && *b == season.number),
            || page::season::page(*series_id, season.number),
        )
    }
}

/// Helper for building menu items.
fn menu_item<E, M, P>(at: &Page, element: E, m: M, page: P) -> w::Button<'static, Message>
where
    Element<'static, Message>: From<E>,
    M: FnOnce(&Page) -> bool,
    P: FnOnce() -> Page,
{
    let current = link(element).width(Length::Fill);

    let current = if m(at) {
        current
    } else {
        current.on_press(Message::Navigate(page()))
    };

    current.width(Length::Shrink)
}
