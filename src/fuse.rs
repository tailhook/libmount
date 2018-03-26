use std::fmt;
use std::fs::{OpenOptions, File};
use std::io;
use std::str::from_utf8;
use std::os::unix::io::AsRawFd;
use std::ffi::CString;
use std::path::Path;

use libc::{uid_t, gid_t, c_int, mode_t, c_char, c_void};
use libc::{mount, getuid, getgid};
use nix::mount::{self as flags, MsFlags};

use {OSError, Error};
use util::{path_to_cstring, as_path};
use explain::{Explainable, exists, user};

/// A fuse mount defintions
#[derive(Debug, Clone)]
pub struct Fuse {
    target: CString,
    mode: Option<mode_t>,
    uid: uid_t,
    gid: gid_t,
    flags: MsFlags,
}

impl Fuse {
    /// Create fuse filesystem mount with default options and current uid/gid
    pub fn new<P: AsRef<Path>>(path: P) -> Fuse {
        Fuse {
            target: path_to_cstring(path.as_ref()),
            mode: None,
            uid: unsafe { getuid() },
            gid: unsafe { getgid() },
            flags: flags::MS_NOSUID|flags::MS_NODEV,
        }
    }
    /// Set initial permissions of the root directory
    pub fn mode(mut self, mode: mode_t) -> Fuse {
        self.mode = Some(mode);
        self
    }
    fn format_options(&self, fd: c_int) -> Vec<u8> {
        use std::io::Write;

        let mut buf = Vec::new();
        write!(&mut buf, "fd={},user_id={},group_id={}",
            fd, self.uid, self.gid).unwrap();
        if let Some(mode) = self.mode {
            if buf.len() != 0 {
                buf.write(b",").unwrap();
            }
            write!(buf, "rootmode={:04o}", mode).unwrap();
        }
        return buf;
    }
    /// Mount the fuse fs
    pub fn bare_mount(self) -> Result<File, OSError> {
        let file = OpenOptions::new().read(true).write(true)
            .open("/dev/fuse");
        let file = match file {
            Ok(f) => f,
            Err(e) => return Err(OSError::from_io(e, Box::new(self)))?,
        };
        let mut options = self.format_options(file.as_raw_fd());
        options.push(0);
        let rc = unsafe { mount(
                b"fuse\0".as_ptr() as *const c_char,
                self.target.as_ptr(),
                b"fuse\0".as_ptr() as *const c_char,
                self.flags.bits(),
                options.as_ptr() as *const c_void) };
        if rc < 0 {
            Err(OSError::from_io(io::Error::last_os_error(), Box::new(self)))
        } else {
            Ok(file)
        }
    }
    /// Set the owner of the filesystem
    pub fn uid(mut self, uid: uid_t) -> Fuse {
        self.uid = uid;
        self
    }
    /// Set the owner of the filesystem
    pub fn gid(mut self, gid: gid_t) -> Fuse {
        self.gid = gid;
        self
    }

    /// Mount the tmpfs and explain error immediately
    pub fn mount(self) -> Result<File, Error> {
        self.bare_mount().map_err(OSError::explain)
    }
}

impl Explainable for Fuse {
    fn explain(&self) -> String {
        [
            format!("target: {}", exists(as_path(&self.target))),
            format!("{}", user()),
        ].join(", ")
    }
}

impl fmt::Display for Fuse {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let opts = self.format_options(-1);
        write!(fmt, "tmpfs {} -> {:?}", from_utf8(&opts).unwrap(),
            as_path(&self.target))
    }
}
