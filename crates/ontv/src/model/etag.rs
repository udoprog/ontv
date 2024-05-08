use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use bstr::BStr;
use serde::de;
use serde::ser;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct Etag(Box<[u8]>);

impl Etag {
    /// Construct a transparent etag.
    #[inline]
    pub(crate) fn new<B>(bytes: &B) -> Self
    where
        B: ?Sized + AsRef<[u8]>,
    {
        Self(bytes.as_ref().into())
    }

    /// Get bytes of the etag.
    #[inline]
    pub(crate) fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }

    #[inline]
    pub(crate) fn as_base64(&self) -> String {
        STANDARD.encode(self.0.as_ref())
    }
}

impl fmt::Display for Etag {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        BStr::new(self.0.as_ref()).fmt(f)
    }
}

impl fmt::Debug for Etag {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        BStr::new(self.0.as_ref()).fmt(f)
    }
}

impl<'de> de::Deserialize<'de> for Etag {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(Visitor)
    }
}

impl ser::Serialize for Etag {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_base64().serialize(serializer)
    }
}

struct Visitor;

impl<'de> de::Visitor<'de> for Visitor {
    type Value = Etag;

    #[inline]
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an etag")
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bytes = STANDARD.decode(v).map_err(de::Error::custom)?;
        Ok(Etag(bytes.into()))
    }

    #[inline]
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bytes = STANDARD.decode(v).map_err(de::Error::custom)?;
        Ok(Etag(bytes.into()))
    }
}
