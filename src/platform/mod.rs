//! OS integration layer.
//!
//! Everything that only exists on one target lives here, behind a uniform API,
//! so the rest of the crate never has to write `#[cfg]` inline. Each backend
//! exposes the same surface; the `not(windows)` one degrades explicitly rather
//! than silently doing nothing.

pub mod single_instance;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use self::unix::*;
