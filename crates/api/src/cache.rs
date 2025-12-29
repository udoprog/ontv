use core::hash::{Hash, Hasher};

/// Generate a 16-byte hash.
pub fn hash16<T>(value: &T) -> [u8; 16]
where
    T: ?Sized + Hash,
{
    const SEED1: u64 = 0x1234567890abcdef;
    const SEED2: u64 = 0xfedcba0987654321;

    struct HasherImpl {
        a: twox_hash::XxHash64,
        b: twox_hash::XxHash64,
    }

    impl Hasher for HasherImpl {
        #[inline]
        fn finish(&self) -> u64 {
            self.a.finish()
        }

        #[inline]
        fn write(&mut self, bytes: &[u8]) {
            self.a.write(bytes);
            self.b.write(bytes);
        }
    }

    let mut hasher = HasherImpl {
        a: twox_hash::XxHash64::with_seed(SEED1),
        b: twox_hash::XxHash64::with_seed(SEED2),
    };

    Hash::hash(value, &mut hasher);
    let a = hasher.a.finish();
    let b = hasher.b.finish();
    let [a0, a1, a2, a3, a4, a5, a6, a7] = a.to_le_bytes();
    let [b0, b1, b2, b3, b4, b5, b6, b7] = b.to_le_bytes();
    [
        a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7,
    ]
}
