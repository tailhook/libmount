use std::fmt;
use std::ffi::{CStr, CString};
use std::path::Path;

use nix::mount::{MsFlags, mount};

use {OSError, Error};
use util::{path_to_cstring, as_path};
use explain::{Explainable, exists};

/// A move operation definition
///
/// This is a similar to `mount --move` and allows to atomically move mount
/// point from one place to another
#[derive(Debug, Clone)]
pub struct Move {
    source: CString,
    target: CString,
}

impl Move {
    /// Create a new Move operation
    pub fn new<A: AsRef<Path>, B: AsRef<Path>>(source: A, target: B) -> Move {
        Move {
            source: path_to_cstring(source.as_ref()),
            target: path_to_cstring(target.as_ref()),
        }
    }

    /// Execute a move-mountpoint operation
    pub fn bare_move_mountpoint(self)
        -> Result<(), OSError>
    {
        mount(Some(&*self.source), &*self.target, None::<&CStr>, MsFlags::MS_MOVE, None::<&CStr>)
            .map_err(|err| OSError::from_nix(err, Box::new(self)))
    }

    /// Execute a move mountpoint operation and explain the error immediately
    pub fn move_mountpoint(self) -> Result<(), Error> {
        self.bare_move_mountpoint().map_err(OSError::explain)
    }
}

impl fmt::Display for Move {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "move {:?} -> {:?}",
            as_path(&self.source), as_path(&self.target))
    }
}

impl Explainable for Move {
    fn explain(&self) -> String {
        [
            format!("source: {}", exists(as_path(&self.source))),
            format!("target: {}", exists(as_path(&self.target))),
        ].join(", ")
    }
}

