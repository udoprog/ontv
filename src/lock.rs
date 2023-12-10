#[cfg(windows)]
mod sys {
    use core::ptr;
    use std::ffi::CString;
    use std::io;

    use winctx::NamedMutex;

    pub struct Lock {
        handle: NamedMutex,
    }

    pub fn try_global_lock(name: &str) -> io::Result<Option<Lock>> {
        match NamedMutex::create_acquired(name)? {
            Some(handle) => Ok(Some(Lock { handle })),
            None => Ok(None),
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use std::io;

    pub struct Lock;

    impl Drop for Lock {
        fn drop(&mut self) {}
    }

    pub fn try_global_lock(_: &str) -> io::Result<Option<Lock>> {
        Ok(Some(Lock))
    }
}

#[doc(inline)]
pub use self::sys::*;
