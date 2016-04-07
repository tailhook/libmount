//! libmount
//!
//! This library has two major goals:
//!
//! 1. Add type-safe interface for mount() system call
//! 2. Add very good explanation of what's wrong when the call fails
//!
//! So we have two error types:
//!
//! 1. Holds mount info and errno
//! 2. Other that is returned by `.explain()` from the first.
//!
//! Note: you need to explain as fast as possible, because during explain
//! library makes some probes for different things in filesystem, and if
//! anything changes it may give incorrect results.
//!
//! You should always `explain()` the errors, unless you are trying lots of
//! mounts for bruteforcing or other similar thing and you are concerned of
//! performance. Usually library does `stat()` and similar things which are
//! much faster than mount anyway. Also explaining is zero-cost in the success
//! path.

extern crate libc;
extern crate nix;
extern crate log;

mod util;
mod error;
mod explain;
mod bind;

use std::io;

use explain::Explainable;
pub use bind::BindMount;

#[derive(Debug)]
pub struct OSError(io::Error, Box<Explainable>);

#[derive(Debug)]
pub struct Error(Box<Explainable>, io::Error, String);
