use std::fmt;

use base64::display::Base64Display;
use base64::engine::DEFAULT_ENGINE;
use serde::de;
use serde::ser;

#[derive(Clone)]
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

    #[inline]
    pub(crate) fn as_base64(&self) -> String {
        base64::encode(self.0.as_ref())
    }
}

impl fmt::Display for Etag {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Base64Display::from(self.0.as_ref(), &DEFAULT_ENGINE).fmt(f)
    }
}

impl fmt::Debug for Etag {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct WithDisplay<T>(T);

        impl<T> fmt::Debug for WithDisplay<T>
        where
            T: fmt::Display,
        {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        let field = WithDisplay(Base64Display::from(self.0.as_ref(), &DEFAULT_ENGINE));
        f.debug_tuple("Etag").field(&field).finish()
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
        write!(f, "an identifier")
    }

    #[inline]
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bytes = base64::decode(v).map_err(de::Error::custom)?;
        Ok(Etag(bytes.into()))
    }

    #[inline]
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bytes = base64::decode(v).map_err(de::Error::custom)?;
        Ok(Etag(bytes.into()))
    }
}
