use std::fmt;
use std::str::Utf8Error;

/// Errors raised in this application.
pub struct Error {
    error: anyhow::Error,
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl From<anyhow::Error> for Error {
    #[inline]
    fn from(error: anyhow::Error) -> Self {
        Self { error }
    }
}

impl From<&'static str> for Error {
    #[inline]
    fn from(value: &'static str) -> Self {
        Self {
            error: anyhow::Error::msg(value),
        }
    }
}

impl From<wasm_bindgen::JsValue> for Error {
    #[inline]
    fn from(value: wasm_bindgen::JsValue) -> Self {
        Self {
            error: anyhow::Error::msg(format!("{:?}", value)),
        }
    }
}

impl From<url::ParseError> for Error {
    #[inline]
    fn from(error: url::ParseError) -> Self {
        Self {
            error: anyhow::Error::from(error),
        }
    }
}

impl From<musli_web::web::Error> for Error {
    #[inline]
    fn from(error: musli_web::web::Error) -> Self {
        Self {
            error: anyhow::Error::from(error),
        }
    }
}

impl From<Utf8Error> for Error {
    #[inline]
    fn from(error: Utf8Error) -> Self {
        Self {
            error: anyhow::Error::from(error),
        }
    }
}
