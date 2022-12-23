use std::fmt;

use anyhow::Error;

use crate::model::{SeasonNumber, SeriesId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    SeriesList,
    Series(SeriesId),
    Settings,
    Season(SeriesId, SeasonNumber),
    Queue,
}

/// A detailed error message.
#[derive(Debug, Clone)]
pub(crate) struct ErrorMessage {
    pub(crate) message: String,
    pub(crate) causes: Vec<String>,
}

impl From<Error> for ErrorMessage {
    fn from(error: Error) -> Self {
        let message = error.to_string();

        let mut causes = Vec::new();

        for cause in error.chain().skip(1) {
            causes.push(cause.to_string());
        }

        ErrorMessage { message, causes }
    }
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.causes.is_empty() {
            return self.message.fmt(f);
        }

        writeln!(f, "{}", self.message)?;

        for cause in &self.causes {
            writeln!(f, "caused by: {cause}")?;
        }

        Ok(())
    }
}
