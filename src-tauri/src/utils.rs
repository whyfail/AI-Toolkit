/// Extension trait to suppress console window creation on Windows.
///
/// On Windows, `std::process::Command` and `tokio::process::Command` default
/// to creating a new console window for each child process. This trait adds
/// the `CREATE_NO_WINDOW` (0x08000000) creation flag to prevent that.
///
/// On non-Windows platforms, `suppress_console()` is a no-op.

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
pub trait SuppressConsole: Sized {
    fn suppress_console(&mut self) -> &mut Self;
}

#[cfg(not(windows))]
pub trait SuppressConsole: Sized {
    fn suppress_console(&mut self) -> &mut Self {
        self
    }
}

// ---------------------------------------------------------------------------
// std::process::Command
// ---------------------------------------------------------------------------

#[cfg(windows)]
impl SuppressConsole for std::process::Command {
    fn suppress_console(&mut self) -> &mut Self {
        self.creation_flags(0x08000000); // CREATE_NO_WINDOW
        self
    }
}

#[cfg(not(windows))]
impl SuppressConsole for std::process::Command {}

// ---------------------------------------------------------------------------
// tokio::process::Command
// ---------------------------------------------------------------------------

#[cfg(windows)]
impl SuppressConsole for tokio::process::Command {
    fn suppress_console(&mut self) -> &mut Self {
        self.creation_flags(0x08000000); // CREATE_NO_WINDOW
        self
    }
}

#[cfg(not(windows))]
impl SuppressConsole for tokio::process::Command {}
