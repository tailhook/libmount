use std::io;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::ptr::null;
use std::path::{Path, PathBuf};
use std::env::current_dir;
use std::error::Error as StdError;
use std::default::Default;

use libc::{mount, c_ulong};
use nix::mount as ms_flags;
use nix::mount::MsFlags;

use {OSError, Error};
use util::path_to_cstring;
use explain::{Explainable, exists, user};
use mountinfo::{MountInfoParser, ParseError};

/// A remount definition
///
/// Usually it is used to change mount flags for a mounted filesystem.
/// Especially to make a readonly filesystem writable or vice versa.
#[derive(Debug, Clone)]
pub struct Remount {
    path: PathBuf,
    flags: MountFlags,
}

#[derive(Debug, Clone, Default)]
struct MountFlags {
    pub readonly: Option<bool>,
    pub nodev: Option<bool>,
    pub noexec: Option<bool>,
    pub nosuid: Option<bool>,
    pub noatime: Option<bool>,
    pub nodiratime: Option<bool>,
    pub relatime: Option<bool>,
    pub strictatime: Option<bool>,
    pub dirsync: Option<bool>,
    pub synchronous: Option<bool>,
    pub mandlock: Option<bool>,
}

impl MountFlags {
    fn apply_to_flags(&self, flags: MsFlags) -> MsFlags {
        let mut flags = flags;
        flags = apply_flag(flags, ms_flags::MS_RDONLY, self.readonly);
        flags = apply_flag(flags, ms_flags::MS_NODEV, self.nodev);
        flags = apply_flag(flags, ms_flags::MS_NOEXEC, self.noexec);
        flags = apply_flag(flags, ms_flags::MS_NOSUID, self.nosuid);
        flags = apply_flag(flags, ms_flags::MS_NOATIME, self.noatime);
        flags = apply_flag(flags, ms_flags::MS_NODIRATIME, self.nodiratime);
        flags = apply_flag(flags, ms_flags::MS_RELATIME, self.relatime);
        flags = apply_flag(flags, ms_flags::MS_STRICTATIME, self.strictatime);
        flags = apply_flag(flags, ms_flags::MS_DIRSYNC, self.dirsync);
        flags = apply_flag(flags, ms_flags::MS_SYNCHRONOUS, self.synchronous);
        flags = apply_flag(flags, ms_flags::MS_MANDLOCK, self.mandlock);
        flags
    }
}

fn apply_flag(flags: MsFlags, flag: MsFlags, set: Option<bool>) -> MsFlags {
    match set {
        Some(true) => flags | flag,
        Some(false) => flags & !flag,
        None => flags,
    }
}

#[derive(Debug)]
pub enum RemountError {
    Io(String, io::Error),
    Os(OSError),
    Mount(Error),
    ParseMountInfo(ParseError),
    UnknownMountPoint(PathBuf),
}

impl fmt::Display for RemountError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::RemountError::*;

        match self {
            &Io(ref msg, ref e) => write!(fmt, "{}: {}", msg, e),
            &Os(ref e) => write!(fmt, "{}", e),
            &Mount(ref e) => write!(fmt, "{}", e),
            &ParseMountInfo(ref msg) => write!(fmt, "{}", msg),
            &UnknownMountPoint(ref p) => {
                write!(fmt, "Cannot find mount point: {:?}", p)
            },
        }
    }
}

impl StdError for RemountError {
    fn cause(&self) -> Option<&StdError> {
        use self::RemountError::*;

        match self {
            &Io(_, ref e) => Some(e),
            &Os(ref e) => Some(e),
            &Mount(ref e) => Some(e),
            &ParseMountInfo(ref e) => Some(e),
            &UnknownMountPoint(_) => None,
        }
    }

    fn description(&self) -> &str {
        use self::RemountError::*;

        match self {
            &Io(_, ref e) => e.description(),
            &Os(ref e) => e.description(),
            &Mount(ref e) => e.description(),
            &ParseMountInfo(ref e) => e.description(),
            &UnknownMountPoint(_) => "Unknown mount point",
        }
    }
}

impl From<io::Error> for RemountError {
    fn from(error: io::Error) -> Self {
        RemountError::Io(format!("{:?}", error), error)
    }
}

impl From<ParseError> for RemountError {
    fn from(error: ParseError) -> Self {
        RemountError::ParseMountInfo(error)
    }
}

impl From<OSError> for RemountError {
    fn from(error: OSError) -> Self {
        RemountError::Os(error)
    }
}

impl From<Error> for RemountError {
    fn from(error: Error) -> Self {
        RemountError::Mount(error)
    }
}

impl Remount {
    pub fn new<A: AsRef<Path>>(path: A) -> Remount {
        Remount {
            path: path.as_ref().to_path_buf(),
            flags: Default::default(),
        }
    }
    /// Set readonly flag
    pub fn readonly(mut self, flag: bool) -> Remount {
        self.flags.readonly = Some(flag);
        self
    }
    /// Set nodev flag
    pub fn nodev(mut self, flag: bool) -> Remount {
        self.flags.nodev = Some(flag);
        self
    }
    /// Set noexec flag
    pub fn noexec(mut self, flag: bool) -> Remount {
        self.flags.noexec = Some(flag);
        self
    }
    /// Set nosuid flag
    pub fn nosuid(mut self, flag: bool) -> Remount {
        self.flags.nosuid = Some(flag);
        self
    }
    /// Set noatime flag
    pub fn noatime(mut self, flag: bool) -> Remount {
        self.flags.noatime = Some(flag);
        self
    }
    /// Set nodiratime flag
    pub fn nodiratime(mut self, flag: bool) -> Remount {
        self.flags.nodiratime = Some(flag);
        self
    }
    /// Set relatime flag
    pub fn relatime(mut self, flag: bool) -> Remount {
        self.flags.relatime = Some(flag);
        self
    }
    /// Set strictatime flag
    pub fn strictatime(mut self, flag: bool) -> Remount {
        self.flags.strictatime = Some(flag);
        self
    }
    /// Set dirsync flag
    pub fn dirsync(mut self, flag: bool) -> Remount {
        self.flags.dirsync = Some(flag);
        self
    }
    /// Set synchronous flag
    pub fn synchronous(mut self, flag: bool) -> Remount {
        self.flags.synchronous = Some(flag);
        self
    }
    /// Set mandlock flag
    pub fn mandlock(mut self, flag: bool) -> Remount {
        self.flags.mandlock = Some(flag);
        self
    }

    /// Execute a remount
    pub fn bare_remount(self) -> Result<(), RemountError> {
        let mut flags = try!(get_mountpoint_flags(&self.path));
        flags = self.flags.apply_to_flags(flags) | ms_flags::MS_REMOUNT;
        let rc = unsafe { mount(
            null(), path_to_cstring(&self.path).as_ptr(),
            null(),
            flags.bits(),
            null()) };
        if rc < 0 {
            Err(RemountError::from(
                OSError(io::Error::last_os_error(), Box::new(self))))
        } else {
            Ok(())
        }
    }

    /// Execute a remount and explain the error immediately
    pub fn remount(self) -> Result<(), RemountError> {
        let mount_res = self.bare_remount();
        match mount_res {
            Err(RemountError::Os(os_error)) => {
                Err(RemountError::from(OSError::explain(os_error)))
            },
            _ => mount_res,
        }
    }
}

impl fmt::Display for MountFlags {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut prefix = "";
        if let Some(true) = self.readonly {
            try!(write!(fmt, "{}ro", prefix));
            prefix = ",";
        }
        if let Some(true) = self.nodev {
            try!(write!(fmt, "{}nodev", prefix));
            prefix = ",";
        }
        if let Some(true) = self.noexec {
            try!(write!(fmt, "{}noexec", prefix));
            prefix = ",";
        }
        if let Some(true) = self.nosuid {
            try!(write!(fmt, "{}nosuid", prefix));
            prefix = ",";
        }
        if let Some(true) = self.noatime {
            try!(write!(fmt, "{}noatime", prefix));
            prefix = ",";
        }
        if let Some(true) = self.nodiratime {
            try!(write!(fmt, "{}nodiratime", prefix));
            prefix = ",";
        }
        if let Some(true) = self.relatime {
            try!(write!(fmt, "{}relatime", prefix));
            prefix = ",";
        }
        if let Some(true) = self.strictatime {
            try!(write!(fmt, "{}strictatime", prefix));
            prefix = ",";
        }
        if let Some(true) = self.dirsync {
            try!(write!(fmt, "{}dirsync", prefix));
            prefix = ",";
        }
        if let Some(true) = self.synchronous {
            try!(write!(fmt, "{}sync", prefix));
            prefix = ",";
        }
        if let Some(true) = self.mandlock {
            try!(write!(fmt, "{}mand", prefix));
        }
        Ok(())
    }
}

impl fmt::Display for Remount {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if !self.flags.apply_to_flags(MsFlags::empty()).is_empty() {
            try!(write!(fmt, "{} ", self.flags));
        }
        write!(fmt, "remount {:?}", &self.path)
    }
}

impl Explainable for Remount {
    fn explain(&self) -> String {
        [
            format!("path: {}", exists(&self.path)),
            format!("{}", user()),
        ].join(", ")
    }
}

fn get_mountpoint_flags(path: &Path) -> Result<MsFlags, RemountError> {
    let mount_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let mut mpath = try!(current_dir());
        mpath.push(path);
        mpath
    };
    let mut mountinfo_content = vec!();
    let mountinfo_path = Path::new("/proc/self/mountinfo");
    let mut mountinfo_file = try!(File::open(mountinfo_path)
        .map_err(|e| RemountError::Io(
            format!("Cannot open file: {:?}", mountinfo_path), e)));
    try!(mountinfo_file.read_to_end(&mut mountinfo_content)
        .map_err(|e| RemountError::Io(
            format!("Cannot read file: {:?}", mountinfo_path), e)));
    match get_mountpoint_flags_from(&mountinfo_content, &mount_path) {
        Ok(Some(flags)) => Ok(MsFlags::from_bits_truncate(flags)),
        Ok(None) => Err(RemountError::UnknownMountPoint(mount_path)),
        Err(e) => Err(e),
    }
}

fn get_mountpoint_flags_from(content: &[u8], path: &Path)
    -> Result<Option<c_ulong>, RemountError>
{
    let mounts_parser = MountInfoParser::new(content);
    let mut flags = None;
    for mount_info_res in mounts_parser {
        let mount_info = try!(mount_info_res);
        if mount_info.mount_point == path {
            flags = Some(mount_info.get_flags());
        }
    }
    if flags.is_some() {
        Ok(flags)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    use libc::{MS_RDONLY, MS_NODEV, MS_NOEXEC, MS_NOSUID};
    use libc::{MS_NOATIME, MS_NODIRATIME, MS_RELATIME, MS_STRICTATIME};
    use libc::{MS_DIRSYNC, MS_SYNCHRONOUS, MS_MANDLOCK};
    use nix::mount::MsFlags;

    use super::{Remount, RemountError, MountFlags};
    use super::{get_mountpoint_flags, get_mountpoint_flags_from};

    #[test]
    fn test_mount_flags() {
        let flags = MountFlags {
            readonly: Some(true),
            nodev: Some(true),
            noexec: Some(true),
            nosuid: Some(true),
            noatime: Some(true),
            nodiratime: Some(true),
            relatime: Some(true),
            strictatime: Some(true),
            dirsync: Some(true),
            synchronous: Some(true),
            mandlock: Some(true),
        };
        let bits = MS_RDONLY | MS_NODEV | MS_NOEXEC | MS_NOSUID |
            MS_NOATIME | MS_NODIRATIME | MS_RELATIME | MS_STRICTATIME |
            MS_DIRSYNC | MS_SYNCHRONOUS | MS_MANDLOCK;
        assert_eq!(flags.apply_to_flags(MsFlags::empty()).bits(), bits);

        let flags = MountFlags {
            readonly: Some(false),
            nodev: Some(false),
            noexec: Some(false),
            nosuid: Some(false),
            noatime: Some(false),
            nodiratime: Some(false),
            relatime: Some(false),
            strictatime: Some(false),
            dirsync: Some(false),
            synchronous: Some(false),
            mandlock: Some(false),
        };
        assert_eq!(flags.apply_to_flags(MsFlags::from_bits_truncate(bits)).bits(), 0);

        let flags = MountFlags::default();
        assert_eq!(flags.apply_to_flags(MsFlags::from_bits_truncate(0)).bits(), 0);
        assert_eq!(flags.apply_to_flags(MsFlags::from_bits_truncate(bits)).bits(), bits);
    }

    #[test]
    fn test_remount() {
        let remount = Remount::new("/");
        assert_eq!(format!("{}", remount), "remount \"/\"");

        let remount = Remount::new("/").readonly(true).nodev(true);
        assert_eq!(format!("{}", remount), "ro,nodev remount \"/\"");
    }

    #[test]
    fn test_get_mountpoint_flags_from() {
        let content = b"19 24 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let flags = get_mountpoint_flags_from(&content[..], Path::new("/proc")).unwrap().unwrap();
        assert_eq!(flags, MS_NODEV | MS_NOEXEC | MS_NOSUID | MS_RELATIME);
    }

    #[test]
    fn test_get_mountpoint_flags_from_dups() {
        let content = b"11 18 0:4 / /tmp rw shared:28 - tmpfs tmpfs rw\n\
                        12 18 0:6 / /tmp rw,nosuid,nodev shared:29 - tmpfs tmpfs rw\n";
        let flags = get_mountpoint_flags_from(&content[..], Path::new("/tmp")).unwrap().unwrap();
        assert_eq!(flags, MS_NOSUID | MS_NODEV);
    }

    #[test]
    fn test_get_mountpoint_flags() {
        assert!(get_mountpoint_flags(Path::new("/dev")).is_ok());
    }

    #[test]
    fn test_get_mountpoint_flags_unknown() {
        let mount_point = Path::new(OsStr::from_bytes(b"/xff"));
        let error = get_mountpoint_flags(mount_point).unwrap_err();
        match error {
            RemountError::UnknownMountPoint(p) => assert_eq!(p, mount_point),
            _ => panic!(),
        }
    }
}
