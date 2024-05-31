use libc::ioctl;
use libc::{c_ushort, STDOUT_FILENO, TIOCGWINSZ};
use termsize::Size;

// NOTE: the `termsize` library has merged a fix but is not released
// See: https://github.com/softprops/termsize/issues/19

/// A representation of the size of the current terminal
#[repr(C)]
#[derive(Debug)]
struct UnixSize {
    /// number of rows
    pub rows: c_ushort,
    /// number of columns
    pub cols: c_ushort,
    x: c_ushort,
    y: c_ushort,
}

/// Gets the current terminal size
#[cfg(unix)]
fn fixed_nix_get() -> Option<Size> {
    // http://rosettacode.org/wiki/Terminal_control/Dimensions#Library:_BSD_libc
    if atty::isnt(atty::Stream::Stdout) {
        return None;
    }
    let mut us = UnixSize {
        rows: 0,
        cols: 0,
        x: 0,
        y: 0,
    };
    let r = unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ.into(), &mut us) };
    if r == 0 {
        Some(Size {
            rows: us.rows,
            cols: us.cols,
        })
    } else {
        None
    }
}

#[cfg(unix)]
pub fn get() -> Option<Size> {
    fixed_nix_get()
}

#[cfg(not(unix))]
pub fn get() -> Option<Size> {
    termsize::get()
}
