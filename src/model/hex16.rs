use std::fmt;

use serde::de;
use serde::ser;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Hex16([u8; 16]);

impl Hex16 {
    /// Construct an ID from bytes.
    pub(crate) fn from_hex<B>(bytes: &B) -> Option<Self>
    where
        B: ?Sized + AsRef<[u8]>,
    {
        let mut out = [0; 16];

        for (&b, to) in bytes.as_ref().iter().rev().zip((0..=31).rev()) {
            let (base, add) = match b {
                b'0'..=b'9' => (b'0', 0),
                b'a'..=b'f' => (b'a', 0xa),
                b'A'..=b'F' => (b'A', 0xa),
                _ => return None,
            };

            out[to / 2] |= (b - base + add) << 4 * u8::from(to % 2 == 0);
        }

        Some(Self(out))
    }
}

impl From<u128> for Hex16 {
    #[inline]
    fn from(value: u128) -> Self {
        Self(value.to_be_bytes())
    }
}

impl fmt::Display for Hex16 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self
            .0
            .iter()
            .flat_map(|b| [*b >> 4, *b & 0b1111])
            .skip_while(|b| *b == 0)
        {
            let b = match b {
                0xa..=0xf => b'a' + (b - 0xa),
                _ => b'0' + b,
            };

            (b as char).fmt(f)?;
        }

        Ok(())
    }
}

impl fmt::Debug for Hex16 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("0x")?;
        fmt::Display::fmt(self, f)
    }
}

impl<'de> de::Deserialize<'de> for Hex16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_bytes(Visitor);

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Hex16;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an identifier")
            }

            #[inline]
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Hex16::from_hex(v) {
                    Some(id) => Ok(id),
                    None => Err(E::custom("bad identifier")),
                }
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Hex16::from_hex(v) {
                    Some(id) => Ok(id),
                    None => Err(E::custom("bad identifier")),
                }
            }
        }
    }
}

impl ser::Serialize for Hex16 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Hex16;

    #[test]
    fn test_hex16() {
        let id = Hex16::from(0x61d78641a649a);
        assert_eq!(id.to_string(), "61d78641a649a");
        let id = Hex16::from_hex(b"61d78641a649a").unwrap();
        assert_eq!(id.to_string(), "61d78641a649a");
        let id = Hex16::from_hex("61d78641a649a").unwrap();
        assert_eq!(id.to_string(), "61d78641a649a");
    }
}
