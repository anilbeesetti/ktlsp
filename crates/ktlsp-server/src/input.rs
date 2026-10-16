//! Owned native stdin without a background reader or an unbounded input queue.
use std::{
    fs::File,
    io::{self, Read},
};

/// A duplicated stdin handle. Reads yield WouldBlock at least every 10 ms when
/// idle, so the single session owner can publish worker completions. Windows
/// supports redirected pipe input, not an interactive console.
pub struct StdinInput(File);
impl StdinInput {
    pub fn new() -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::fd::AsFd;
            Ok(Self(File::from(io::stdin().as_fd().try_clone_to_owned()?)))
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::{AsHandle, AsRawHandle};
            let file = File::from(io::stdin().as_handle().try_clone_to_owned()?);
            // SAFETY: file owns a live handle; GetFileType does not retain it.
            if unsafe { GetFileType(file.as_raw_handle()) } != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "stdio requires redirected pipe input",
                ));
            }
            Ok(Self(file))
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "unsupported native stdin platform",
            ))
        }
    }
}
impl Read for StdinInput {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let mut fd = PollFd {
                fd: self.0.as_raw_fd(),
                events: 1,
                revents: 0,
            };
            // SAFETY: fd is initialized and writable for one pollfd; the file
            // stays owned throughout this call. poll does not retain pointers.
            match unsafe { poll(&mut fd, 1, 10) } {
                -1 => return Err(io::Error::last_os_error()),
                0 => return Err(io::ErrorKind::WouldBlock.into()),
                _ => {}
            }
            self.0.read(bytes)
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let mut available = 0;
            // SAFETY: the owned pipe handle is live and available points to a
            // writable u32. No buffer is requested and nothing is retained.
            if unsafe {
                PeekNamedPipe(
                    self.0.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                let error = io::Error::last_os_error();
                return if matches!(error.raw_os_error(), Some(109 | 232)) {
                    Ok(0)
                } else {
                    Err(error)
                };
            }
            if available == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let count = bytes.len().min(available as usize);
            self.0.read(&mut bytes[..count])
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

#[cfg(unix)]
#[repr(C)]
struct PollFd {
    fd: std::ffi::c_int,
    events: std::ffi::c_short,
    revents: std::ffi::c_short,
}
#[cfg(target_os = "linux")]
type PollCount = std::ffi::c_ulong;
#[cfg(all(unix, not(target_os = "linux")))]
type PollCount = std::ffi::c_uint;
#[cfg(unix)]
unsafe extern "C" {
    fn poll(fds: *mut PollFd, count: PollCount, timeout: std::ffi::c_int) -> std::ffi::c_int;
}
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileType(handle: std::os::windows::io::RawHandle) -> u32;
    fn PeekNamedPipe(
        handle: std::os::windows::io::RawHandle,
        buffer: *mut std::ffi::c_void,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        remaining: *mut u32,
    ) -> i32;
}
