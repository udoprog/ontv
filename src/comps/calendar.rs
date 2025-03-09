#![allow(warnings)]

use std::mem;

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    PickDay(NaiveDate),
    PrevMonth,
    NextMonth,
}

pub(crate) struct Calendar {
    start_of_week: Weekday,
    date: NaiveDate,
    month: NaiveDate,
    first: NaiveDate,
    last: NaiveDate,
}

impl Calendar {
    /// Construct a new calendar.
    pub(crate) fn new(date: NaiveDate, start_of_week: Weekday) -> Self {
        let mut this = Self {
            start_of_week,
            date,
            month: date,
            first: date,
            last: date,
        };

        this.build_span(date);
        this
    }

    pub(crate) fn update(&mut self, message: Message) {
        match message {
            Message::PickDay(date) => {
                self.date = date;
                self.build_span(date);
            }
            Message::PrevMonth => {
                if let Some(month) = self.month.checked_sub_months(Months::new(1)) {
                    self.build_span(month);
                }
            }
            Message::NextMonth => {
                if let Some(month) = self.month.checked_add_months(Months::new(1)) {
                    self.build_span(month);
                }
            }
        }
    }

    pub(crate) fn view(&self) -> Element<'static, Message> {
        let mut placeholder = w::Column::new();

        let mut cols = [
            w::Column::new(),
            w::Column::new(),
            w::Column::new(),
            w::Column::new(),
            w::Column::new(),
            w::Column::new(),
            w::Column::new(),
        ];

        for (date, o) in self.week(self.start_of_week).zip(cols.iter_mut()) {
            let mut col = mem::replace(o, placeholder);
            col = col.push(w::text(format_week(date)).size(SMALL_SIZE));
            placeholder = mem::replace(o, col);
        }

        for (date, o) in self
            .first
            .iter_days()
            .take_while(|d| *d <= self.last)
            .zip((0..7).into_iter().cycle())
        {
            let mut button = w::button(
                w::text(date.day())
                    .width(Length::Fill)
                    .size(SMALL_SIZE)
                    .horizontal_alignment(Horizontal::Center),
            );

            if date == self.date {
                button = button.style(w::button::success);
            } else if date.month() != self.month.month() {
                button = button.style(w::button::secondary);
            } else {
                button = button.style(w::button::primary);
            }

            button = button.width(Length::Fill).on_press(Message::PickDay(date));

            let mut col = mem::replace(&mut cols[o], placeholder);
            col = col.push(button);
            placeholder = mem::replace(&mut cols[o], col);
        }

        let mut row = w::Row::new();

        for col in cols {
            row = row.push(
                col.align_items(Alignment::Center)
                    .spacing(SPACE)
                    .width(Length::FillPortion(1)),
            );
        }

        let mut column = w::Column::new();

        let mut title = w::Row::new();

        title =
            title.push(w::button(w::text("Prev").size(SMALL_SIZE)).on_press(Message::PrevMonth));

        title = title.push(
            w::text(format!(
                "{} {}",
                format_month(self.month.month()),
                self.month.year()
            ))
            .horizontal_alignment(Horizontal::Center)
            .width(Length::Fill),
        );

        title =
            title.push(w::button(w::text("Next").size(SMALL_SIZE)).on_press(Message::NextMonth));

        column = column.push(title.spacing(SPACE));
        column = column.push(row.spacing(SPACE));

        column.spacing(SPACE).into()
    }

    fn week(&self, start_of_week: Weekday) -> impl Iterator<Item = Weekday> {
        [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .into_iter()
        .cycle()
        .skip(start_of_week.num_days_from_monday() as usize)
    }

    fn build_span(&mut self, date: NaiveDate) {
        let month = date.with_day(1);
        let last = month.and_then(|d| {
            d.checked_add_months(Months::new(1))?
                .checked_sub_days(Days::new(1))
        });
        let first = month.map(|d| d.week(self.start_of_week).first_day());
        let last = last.map(|d| d.week(self.start_of_week).last_day());

        if let (Some(month), Some(first), Some(last)) = (month, first, last) {
            self.month = month;
            self.first = first;
            self.last = last;
        } else {
            self.month = date;
            self.first = date;
            self.last = date;
        }
    }
}

fn format_week(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Mo",
        Weekday::Tue => "Tu",
        Weekday::Wed => "We",
        Weekday::Thu => "Th",
        Weekday::Fri => "Fr",
        Weekday::Sat => "Sa",
        Weekday::Sun => "Su",
    }
}

fn format_month(m: u32) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}
