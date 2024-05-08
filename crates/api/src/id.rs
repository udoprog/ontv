use core::fmt;
use core::str::FromStr;

use musli::{Decode, Encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            Encode,
            Decode,
        )]
        #[repr(transparent)]
        #[serde(transparent)]
        #[musli(transparent)]
        pub struct $name(#[musli(with = musli::serde)] Uuid);

        impl $name {
            /// Generate a new random series identifier.
            #[inline]
            #[allow(unused)]
            pub fn random() -> Self {
                Self(Uuid::new_v4())
            }

            /// Access underlying id.
            #[inline]
            #[allow(unused)]
            pub fn id(&self) -> &Uuid {
                &self.0
            }
        }

        impl fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(uuid::Uuid::from_str(s)?))
            }
        }
    };
}

id!(SeriesId);
id!(EpisodeId);
id!(MovieId);
id!(WatchedId);
id!(TaskId);
