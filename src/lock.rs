#[cfg(windows)]
mod sys {
    use core::ptr;
    use std::ffi::CString;
    use std::io;

    use windows_sys::core::PCSTR;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, TRUE,
    };
    use windows_sys::Win32::System::Threading::CreateMutexA;

    pub struct Lock {
        handle: HANDLE,
    }

    impl Drop for Lock {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    pub fn try_global_lock(name: &str) -> io::Result<Lock> {
        unsafe {
            let name = CString::new(name)?;
            let handle = CreateMutexA(ptr::null(), TRUE, name.as_ptr() as PCSTR);

            if GetLastError() == ERROR_ALREADY_EXISTS || handle == 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Lock { handle })
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

    pub fn try_global_lock(_: &str) -> io::Result<Lock> {
        Ok(Lock)
    }
}

#[doc(inline)]
pub use self::sys::*;
