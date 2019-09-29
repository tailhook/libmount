use std::io;
use std::fmt;
use std::ffi::CStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::env::current_dir;
use std::default::Default;

use nix::mount::{MsFlags, mount};

use {OSError, Error};
use util::path_to_cstring;
use explain::{Explainable, exists, user};
use mountinfo::{parse_mount_point};

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
    pub bind: Option<bool>,
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
        flags = apply_flag(flags, MsFlags::MS_BIND, self.bind);
        flags = apply_flag(flags, MsFlags::MS_RDONLY, self.readonly);
        flags = apply_flag(flags, MsFlags::MS_NODEV, self.nodev);
        flags = apply_flag(flags, MsFlags::MS_NOEXEC, self.noexec);
        flags = apply_flag(flags, MsFlags::MS_NOSUID, self.nosuid);
        flags = apply_flag(flags, MsFlags::MS_NOATIME, self.noatime);
        flags = apply_flag(flags, MsFlags::MS_NODIRATIME, self.nodiratime);
        flags = apply_flag(flags, MsFlags::MS_RELATIME, self.relatime);
        flags = apply_flag(flags, MsFlags::MS_STRICTATIME, self.strictatime);
        flags = apply_flag(flags, MsFlags::MS_DIRSYNC, self.dirsync);
        flags = apply_flag(flags, MsFlags::MS_SYNCHRONOUS, self.synchronous);
        flags = apply_flag(flags, MsFlags::MS_MANDLOCK, self.mandlock);
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

quick_error! {
    #[derive(Debug)]
    pub enum RemountError {
        Io(msg: String, err: io::Error) {
            cause(err)
            display("{}: {}", msg, err)
            description(err.description())
            from(err: io::Error) -> (String::new(), err)
        }
        ParseMountInfo(err: String) {
            display("{}", err)
            from()
        }
        UnknownMountPoint(path: PathBuf) {
            display("Cannot find mount point: {:?}", path)
        }
    }
}

impl Remount {
    /// Create a new Remount operation
    ///
    /// By default it doesn't modify any flags. So is basically useless, you
    /// should set some flags to make it effective.
    pub fn new<A: AsRef<Path>>(path: A) -> Remount {
        Remount {
            path: path.as_ref().to_path_buf(),
            flags: Default::default(),
        }
    }
    /// Set bind flag
    /// Note: remount readonly doesn't work without MS_BIND flag
    /// inside unpriviledged user namespaces
    pub fn bind(mut self, flag: bool) -> Remount {
        self.flags.bind = Some(flag);
        self
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
    pub fn bare_remount(self) -> Result<(), OSError> {
        let mut flags = match get_mountpoint_flags(&self.path) {
            Ok(flags) => flags,
            Err(e) => {
                return Err(OSError::from_remount(e, Box::new(self)));
            },
        };
        flags = self.flags.apply_to_flags(flags) | MsFlags::MS_REMOUNT;
        mount(
            None::<&CStr>,
            &*path_to_cstring(&self.path),
            None::<&CStr>,
            flags,
            None::<&CStr>,
        ).map_err(|err| OSError::from_nix(err, Box::new(self)))
    }

    /// Execute a remount and explain the error immediately
    pub fn remount(self) -> Result<(), Error> {
        self.bare_remount().map_err(OSError::explain)
    }
}

impl fmt::Display for MountFlags {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut prefix = "";
        if let Some(true) = self.bind {
            try!(write!(fmt, "{}bind", prefix));
            prefix = ",";
        }
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
    let mut mountinfo_content = Vec::with_capacity(4 * 1024);
    let mountinfo_path = Path::new("/proc/self/mountinfo");
    let mut mountinfo_file = try!(File::open(mountinfo_path)
        .map_err(|e| RemountError::Io(
            format!("Cannot open file: {:?}", mountinfo_path), e)));
    try!(mountinfo_file.read_to_end(&mut mountinfo_content)
        .map_err(|e| RemountError::Io(
            format!("Cannot read file: {:?}", mountinfo_path), e)));
    match get_mountpoint_flags_from(&mountinfo_content, &mount_path) {
        Ok(Some(flags)) => Ok(flags),
        Ok(None) => Err(RemountError::UnknownMountPoint(mount_path)),
        Err(e) => Err(e),
    }
}

fn get_mountpoint_flags_from(content: &[u8], path: &Path)
    -> Result<Option<MsFlags>, RemountError>
{
    // iterate from the end of the mountinfo file
    for line in content.split(|c| *c == b'\n').rev() {
        let entry = parse_mount_point(line)
            .map_err(|e| RemountError::ParseMountInfo(e.0))?;
        if let Some(mount_point) = entry {
            if mount_point.mount_point == path {
                return Ok(Some(mount_point.get_mount_flags()));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    use nix::mount::MsFlags;

    use Error;
    use super::{Remount, RemountError, MountFlags};
    use super::{get_mountpoint_flags, get_mountpoint_flags_from};

    #[test]
    fn test_mount_flags() {
        let flags = MountFlags {
            bind: Some(true),
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
        let bits = (MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID |
            MsFlags::MS_NOATIME | MsFlags::MS_NODIRATIME | MsFlags::MS_RELATIME | MsFlags::MS_STRICTATIME |
            MsFlags::MS_DIRSYNC | MsFlags::MS_SYNCHRONOUS | MsFlags::MS_MANDLOCK).bits();
        assert_eq!(flags.apply_to_flags(MsFlags::empty()).bits(), bits);

        let flags = MountFlags {
            bind: Some(false),
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
        assert_eq!(flags, MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_RELATIME);
    }

    #[test]
    fn test_get_mountpoint_flags_from_dups() {
        let content = b"11 18 0:4 / /tmp rw shared:28 - tmpfs tmpfs rw\n\
                        12 18 0:6 / /tmp rw,nosuid,nodev shared:29 - tmpfs tmpfs rw\n";
        let flags = get_mountpoint_flags_from(&content[..], Path::new("/tmp")).unwrap().unwrap();
        assert_eq!(flags, MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
    }

    #[test]
    fn test_get_mountpoint_flags() {
        assert!(get_mountpoint_flags(Path::new("/")).is_ok());
    }

    #[test]
    fn test_get_mountpoint_flags_unknown() {
        let mount_point = Path::new(OsStr::from_bytes(b"/\xff"));
        let error = get_mountpoint_flags(mount_point).unwrap_err();
        match error {
            RemountError::UnknownMountPoint(p) => assert_eq!(p, mount_point),
            _ => panic!(),
        }
    }

    #[test]
    fn test_remount_unknown_mountpoint() {
        let remount = Remount::new("/non-existent");
        let error = remount.remount().unwrap_err();
        let Error(_, e, msg) = error;
        match e.get_ref() {
            Some(e) => {
                assert_eq!(
                   e.to_string(),
                   "Cannot find mount point: \"/non-existent\"");
            },
            _ => panic!(),
        }
        assert!(msg.starts_with(
            "Cannot find mount point: \"/non-existent\", path: missing, "));
    }
}
