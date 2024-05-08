use std::fmt;

use anyhow::Error;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Identifier used to look up errors caused by specific actions..
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ErrorId {
    Search(Uuid),
}

/// A detailed error message.
#[derive(Debug, Clone)]
pub(crate) struct ErrorInfo {
    pub(crate) id: Option<ErrorId>,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) message: String,
    pub(crate) causes: Vec<String>,
}

impl ErrorInfo {
    /// Construt a new error with the given identifier.
    pub(crate) fn new(id: ErrorId, error: Error) -> Self {
        Self::internal(Some(id), error)
    }

    fn internal(id: Option<ErrorId>, error: Error) -> Self {
        let message = error.to_string();

        let mut causes = Vec::new();

        for cause in error.chain().skip(1) {
            causes.push(cause.to_string());
        }

        Self {
            id,
            timestamp: Utc::now(),
            message,
            causes,
        }
    }
}

impl From<Error> for ErrorInfo {
    #[inline]
    fn from(error: Error) -> Self {
        Self::internal(None, error)
    }
}

impl fmt::Display for ErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.causes.is_empty() {
            return self.message.fmt(f);
        }

        write!(f, "{}", self.message)?;

        for cause in &self.causes {
            writeln!(f)?;
            write!(f, "caused by: {cause}")?;
        }

        Ok(())
    }
}
