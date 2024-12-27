use std::fmt;
use std::fmt::Write;

use serde::de;
use serde::ser;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Raw<const N: usize>([u8; N]);

impl<const N: usize> Raw<N> {
    /// Construct a raw identifier from a string.
    pub(crate) fn new<B>(bytes: &B) -> Option<Self>
    where
        B: ?Sized + AsRef<[u8]>,
    {
        let mut out = [0; N];
        let mut it = out.iter_mut();

        for b in bytes.as_ref() {
            *it.next()? = *b;
        }

        Some(Self(out))
    }
}

impl<const N: usize> fmt::Display for Raw<N> {
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

impl<const N: usize> fmt::Debug for Raw<N> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        fmt::Display::fmt(self, f)?;
        f.write_char('"')?;
        Ok(())
    }
}

impl<'de, const N: usize> de::Deserialize<'de> for Raw<N> {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(Visitor)
    }
}

impl<const N: usize> ser::Serialize for Raw<N> {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

struct Visitor<const N: usize>;

impl<const N: usize> de::Visitor<'_> for Visitor<N> {
    type Value = Raw<N>;

    #[inline]
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an raw string")
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Raw::new(v) {
            Some(value) => Ok(value),
            None => Err(E::custom("value overflow")),
        }
    }

    #[inline]
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Raw::new(v) {
            Some(value) => Ok(value),
            None => Err(E::custom("value overflow")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Raw;

    #[test]
    fn test_raw16() {
        let id = Raw::<16>::new(b"foobarbaz").unwrap();
        assert_eq!(id.to_string(), "foobarbaz");
        let id = Raw::<16>::new("foobarbaz").unwrap();
        assert_eq!(id.to_string(), "foobarbaz");
    }
}
