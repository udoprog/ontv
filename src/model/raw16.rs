use std::fmt;

use serde::de;
use serde::ser;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Raw16([u8; 16]);

impl Raw16 {
    /// Construct a raw identifier from a string.
    pub(crate) fn from_string<B>(bytes: &B) -> Self
    where
        B: ?Sized + AsRef<[u8]>,
    {
        let mut out = [0; 16];

        for (&b, o) in bytes.as_ref().iter().zip(out.iter_mut()) {
            *o = b;
        }

        Self(out)
    }
}

impl fmt::Display for Raw16 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = trim_end(&self.0);

        for b in bytes {
            if b.is_ascii_graphic() {
                (*b as char).fmt(f)?;
            }
        }

        return Ok(());

        #[inline]
        fn trim_end(mut bytes: &[u8]) -> &[u8] {
            while let [head @ .., b] = bytes {
                if *b != 0 {
                    break;
                }

                bytes = head;
            }

            bytes
        }
    }
}

impl<'de> de::Deserialize<'de> for Raw16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_bytes(Visitor);

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Raw16;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an identifier")
            }

            #[inline]
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Raw16::from_string(v))
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Raw16::from_string(v))
            }
        }
    }
}

impl ser::Serialize for Raw16 {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}
