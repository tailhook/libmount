//! libmount
//!
//! This library has two major goals:
//!
//! 1. Add type-safe interface for mount() system call
//! 2. Add very good explanation of what's wrong when the call fails
//!
//! So we have two error types:
//!
//! 1. `OSError` holds mount info and errno
//! 2. `Error` is returned by `OSError::explain()`
//!
//! The first one is returned by `bare_mount()` the second by `mount()`, and
//! using latter is preffered for most situations. Unless performance is
//! too critical (i.e. you are doing thousands of *failing* mounts per second).
//! On the success path there is no overhead.
//!

extern crate libc;
extern crate nix;
extern crate log;

mod util;
mod error;
mod explain;
mod bind;
mod overlay;
mod tmpfs;
mod modify;

use std::io;

use explain::Explainable;
pub use bind::BindMount;
pub use overlay::Overlay;
pub use tmpfs::Tmpfs;
pub use modify::Move;

/// The raw os error
///
/// This is a wrapper around `io::Error` providing `explain()` method
///
/// Note: you need to explain as fast as possible, because during explain
/// library makes some probes for different things in filesystem, and if
/// anything changes it may give incorrect results.
///
/// You should always `explain()` the errors, unless you are trying lots of
/// mounts for bruteforcing or other similar thing and you are concerned of
/// performance. Usually library does `stat()` and similar things which are
/// much faster than mount anyway. Also explaining is zero-cost in the success
/// path.
///
#[derive(Debug)]
pub struct OSError(io::Error, Box<Explainable>);

/// The error holder which contains as much information about why failure
/// happens as the library implementors could gain
///
/// This type only provides `Display` for now, but some programmatic interface
/// is expected in future.
#[derive(Debug)]
pub struct Error(Box<Explainable>, io::Error, String);
