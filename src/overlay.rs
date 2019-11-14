use std::fmt;
use std::path::{Path, PathBuf};
use std::fs::metadata;
use std::ffi::{CStr, CString};
use std::os::unix::fs::MetadataExt;
use std::os::unix::ffi::OsStrExt;

use nix::mount::{MsFlags, mount};

use util::{path_to_cstring, as_path};
use {OSError, Error};
use explain::{Explainable, exists, user};


/// An overlay mount point
///
/// This requires linux kernel of at least 3.18.
///
/// To use overlayfs in user namespace you need a kernel patch (which is
/// enabled by default in ubuntu). At least this is still true for mainline
/// kernel 4.5.0.
#[derive(Debug, Clone)]
pub struct Overlay {
    lowerdirs: Vec<PathBuf>,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    target: CString,
}

impl Overlay {
    /// A constructor for read-only overlayfs mount
    ///
    /// You must have at least two directories in the list (for single
    /// dir it might be equal to bind mount, but kernel return EINVAL for
    /// such options).
    ///
    /// The top-most directory will be first in the list.
    pub fn readonly<'x, I, T>(dirs: I, target: T) -> Overlay
        where I: Iterator<Item=&'x Path>, T: AsRef<Path>
    {
        Overlay {
            lowerdirs: dirs.map(|x| x.to_path_buf()).collect(),
            upperdir: None,
            workdir: None,
            target: path_to_cstring(target.as_ref()),
        }
    }
    /// A constructor for writable overlayfs mount
    ///
    /// The upperdir and workdir must be on the same filesystem.
    ///
    /// The top-most directory will be first in the list of lowerdirs.
    pub fn writable<'x, I, B, C, D>(lowerdirs: I, upperdir: B,
                                workdir: C, target: D)
        -> Overlay
        where I: Iterator<Item=&'x Path>, B: AsRef<Path>,
              C: AsRef<Path>, D: AsRef<Path>,
    {
        Overlay {
            lowerdirs: lowerdirs.map(|x| x.to_path_buf()).collect(),
            upperdir: Some(upperdir.as_ref().to_path_buf()),
            workdir: Some(workdir.as_ref().to_path_buf()),
            target: path_to_cstring(target.as_ref()),
        }
    }

    /// Execute an overlay mount
    pub fn bare_mount(self) -> Result<(), OSError> {
        let mut options = Vec::new();
        options.extend(b"lowerdir=");
        for (i, p) in self.lowerdirs.iter().enumerate() {
            if i != 0 {
                options.push(b':')
            }
            append_escape(&mut options, p);
        }
        if let (Some(u), Some(w)) = (self.upperdir.as_ref(), self.workdir.as_ref()) {
            options.extend(b",upperdir=");
            append_escape(&mut options, u);
            options.extend(b",workdir=");
            append_escape(&mut options, w);
        }
        mount(
            Some(CStr::from_bytes_with_nul(b"overlay\0").unwrap()),
            &*self.target,
            Some(CStr::from_bytes_with_nul(b"overlay\0").unwrap()),
            MsFlags::empty(),
            Some(&*options),
        ).map_err(|err| OSError::from_nix(err, Box::new(self)))
    }

    /// Execute an overlay mount and explain the error immediately
    pub fn mount(self) -> Result<(), Error> {
        self.bare_mount().map_err(OSError::explain)
    }
}

/// Escape the path to put it into options string for overlayfs
///
/// The rules here are not documented anywhere as far as I know and was
/// derived experimentally.
fn append_escape(dest: &mut Vec<u8>, path: &Path) {
    for &byte in path.as_os_str().as_bytes().iter() {
        match byte {
            // This is escape char
            b'\\' => { dest.push(b'\\'); dest.push(b'\\'); }
            // This is used as a path separator in lowerdir
            b':' => { dest.push(b'\\'); dest.push(b':'); }
            // This is used as a argument separator
            b',' => { dest.push(b'\\'); dest.push(b','); }
            x => dest.push(x),
        }
    }
}

impl fmt::Display for Overlay {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if let (Some(udir), Some(wdir)) =
                (self.upperdir.as_ref(), self.workdir.as_ref())
        {
            write!(fmt, "overlayfs \
                {},upperdir={:?},workdir={:?} -> {:?}",
                self.lowerdirs.iter().map(|x| format!("{:?}", x))
                    .collect::<Vec<_>>().join(":"),
                udir, wdir, as_path(&self.target))
        } else {
            write!(fmt, "overlayfs \
                {} -> {:?}",
                self.lowerdirs.iter().map(|x| format!("{:?}", x))
                    .collect::<Vec<_>>().join(":"),
                as_path(&self.target))
        }
    }
}

impl Explainable for Overlay {
    fn explain(&self) -> String {
        let mut info = self.lowerdirs.iter()
            .map(|x| format!("{:?}: {}", x, exists(x)))
            .collect::<Vec<String>>();
        if let (Some(udir), Some(wdir)) =
                (self.upperdir.as_ref(), self.workdir.as_ref())
        {
            let umeta = metadata(&udir).ok();
            let wmeta = metadata(&wdir).ok();
            info.push(format!("upperdir: {}", exists(&udir)));
            info.push(format!("workdir: {}", exists(&wdir)));

            if let (Some(u), Some(w)) = (umeta, wmeta) {
                info.push(format!("{}", if u.dev() == w.dev()
                    { "same-fs" } else { "different-fs" }));
            }
            if udir.starts_with(wdir) {
                info.push("upperdir-prefix-of-workdir".to_string());
            } else if wdir.starts_with(udir) {
                info.push("workdir-prefix-of-upperdir".to_string());
            }
            info.push(format!("target: {}", exists(as_path(&self.target))));
        }
        if self.lowerdirs.len() < 1 {
            info.push("no-lowerdirs".to_string());
        } else if self.upperdir.is_none() && self.lowerdirs.len() < 2 {
            info.push("single-lowerdir".to_string());
        }
        info.push(user().to_string());
        info.join(", ")
    }
}

