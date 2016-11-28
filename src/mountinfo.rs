use std::fmt;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::borrow::Cow;
use std::error::Error;

use libc::c_ulong;
use libc::{MS_RDONLY, MS_NOSUID, MS_NODEV, MS_NOEXEC, MS_SYNCHRONOUS};
use libc::{MS_MANDLOCK, MS_DIRSYNC, MS_NOATIME, MS_NODIRATIME};
use libc::{MS_RELATIME, MS_STRICTATIME};

#[derive(Debug)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseError: {}", self.description())
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        return &self.0;
    }
}

pub struct MountInfoParser<'a> {
    data: &'a [u8],
    row_num: usize,
    exhausted: bool,
}

impl<'a> MountInfoParser<'a> {
    pub fn new(data: &'a [u8]) -> MountInfoParser<'a> {
        MountInfoParser {
            data: data,
            row_num: 0,
            exhausted: false,
        }
    }
}

pub struct MountPoint {
    pub mount_id: c_ulong,
    pub parent_id: c_ulong,
    pub major: c_ulong,
    pub minor: c_ulong,
    pub root: PathBuf,
    pub mount_point: PathBuf,
    pub mount_options: OsString,
    pub optional_fields: Vec<OsString>,
    pub fstype: OsString,
    pub mount_source: OsString,
    pub super_options: OsString,
}

impl MountPoint {
    pub fn get_flags(&self) -> c_ulong {
        let mut flags = 0 as c_ulong;
        for opt in self.mount_options.as_bytes().split(|c| *c == b',') {
            let opt = OsStr::from_bytes(opt);
            if opt == OsStr::new("ro") { flags |= MS_RDONLY }
            else if opt == OsStr::new("nosuid") { flags |= MS_NOSUID }
            else if opt == OsStr::new("nodev") { flags |= MS_NODEV }
            else if opt == OsStr::new("noexec") { flags |= MS_NOEXEC }
            else if opt == OsStr::new("mand") { flags |= MS_MANDLOCK }
            else if opt == OsStr::new("sync") { flags |= MS_SYNCHRONOUS }
            else if opt == OsStr::new("dirsync") { flags |= MS_DIRSYNC }
            else if opt == OsStr::new("noatime") { flags |= MS_NOATIME }
            else if opt == OsStr::new("nodiratime") { flags |= MS_NODIRATIME }
            else if opt == OsStr::new("relatime") { flags |= MS_RELATIME }
            else if opt == OsStr::new("strictatime") { flags |= MS_STRICTATIME }
        }
        flags
    }
}

impl<'a> Iterator for MountInfoParser<'a> {
    type Item = Result<MountPoint, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }

        loop {
            match self.data.iter().position(|c| *c == b'\n') {
                Some(ix) => {
                    self.row_num += 1;
                    let row = &self.data[..ix];
                    self.data = &self.data[ix + 1..];
                    let row = if row.ends_with(&[b'\r']) {
                        &row[..row.len() - 1]
                    } else {
                        row
                    };
                    if row.is_empty() {
                        continue;
                    }
                    return Some(parse_mount_info(row));
                },
                None => {
                    self.exhausted = true;
                    if self.data.is_empty() {
                        return None;
                    } else {
                        return Some(parse_mount_info(&self.data));
                    }
                },
            }
        }
    }
}

fn parse_mount_info(row: &[u8]) -> Result<MountPoint, ParseError> {
    let invalid_format = || {
        ParseError(format!("Expected more values"))
    };

    let mut cols = row.split(|c| *c == b' ');
    let mount_id = try!(parse_int(&mut cols, row));
    let parent_id = try!(parse_int(&mut cols, row));
    let mut major_minor = try!(cols.next().ok_or_else(&invalid_format))
        .split(|c| *c == b':');
    let major = try!(parse_int(&mut major_minor, row));
    let minor = try!(parse_int(&mut major_minor, row));
    let root = try!(parse_path(&mut cols, row));
    let mount_point = try!(parse_path(&mut cols, row));
    let mount_options = try!(parse_os_str(&mut cols, row));
    let mut optional_fields = vec!();
    let mut opt_field = try!(parse_os_str(&mut cols, row));
    while opt_field != OsStr::new("-") {
        optional_fields.push(opt_field.clone());
        opt_field = try!(parse_os_str(&mut cols, row));
    }
    let fstype = try!(parse_os_str(&mut cols, &row));
    let mount_source = try!(parse_os_str(&mut cols, &row));
    let super_options = try!(parse_os_str(&mut cols, &row));
    Ok(MountPoint {
        mount_id: mount_id,
        parent_id: parent_id,
        major: major,
        minor: minor,
        root: root,
        mount_point: mount_point,
        mount_options: mount_options,
        optional_fields: optional_fields,
        fstype: fstype,
        mount_source: mount_source,
        super_options: super_options,
    })
}

fn parse_os_str(columns: &mut Iterator<Item=&[u8]>, row: &[u8])
    -> Result<OsString, ParseError>
{
    let bytes = try!(columns.next()
        .ok_or_else(|| ParseError(
            format!("Expected more values in row: {:?}",
                String::from_utf8_lossy(row)))));
    let mut value = Cow::Borrowed(bytes);
    try!(unescape_octals(&mut value));
    Ok(OsString::from_vec(value.into_owned()))
}

fn parse_int(columns: &mut Iterator<Item=&[u8]>, row: &[u8])
    -> Result<c_ulong, ParseError>
{
    let col = try!(columns.next()
        .ok_or_else(|| ParseError(
            format!("Expected more values for row: {:?}",
                String::from_utf8_lossy(row))))
        .map(|v| String::from_utf8_lossy(v)));
    col.parse::<c_ulong>()
        .map_err(|_| ParseError(
            format!("Cannot parse integer from {:?}: {:?}",
                col, String::from_utf8_lossy(row))))
}

fn parse_path(columns: &mut Iterator<Item=&[u8]>, row: &[u8])
    -> Result<PathBuf, ParseError>
{
    Ok(PathBuf::from(try!(parse_os_str(columns, row))))
}

fn unescape_octals(v: &mut Cow<[u8]>) -> Result<(), ParseError>{
    let mut i = 0;
    loop {
        if v[i] == b'\\' {
            let tail = v.to_mut().split_off(i);
            if tail.len() < 4 {
                return Err(ParseError(format!("Invalid escaping")));
            }
            let oct = String::from_utf8_lossy(&tail[1..4]);
            let b = try!(u8::from_str_radix(&oct, 8)
                .map_err(|_| ParseError(format!("Expected octal number"))));
            v.to_mut().push(b);
            v.to_mut().extend_from_slice(&tail[4..]);
        }
        i += 1;
        if i >= v.len() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt;

    use libc::{MS_NOSUID, MS_NODEV, MS_NOEXEC, MS_RELATIME};

    use super::{MountInfoParser, ParseError};

    #[test]
    fn test_mount_info_parser_proc() {
        let content = b"19 24 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 19);
        assert_eq!(mount_point.parent_id, 24);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 4);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,nosuid,nodev,noexec,relatime"));
        assert_eq!(mount_point.optional_fields, vec!(OsStr::new("shared:12")));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_flags(), MS_NOSUID | MS_NODEV | MS_NOEXEC | MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_missing_optional_fields() {
        let content = b"335 294 0:56 / /proc rw,relatime - proc proc rw";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 335);
        assert_eq!(mount_point.parent_id, 294);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 56);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, Vec::new() as Vec<OsString>);
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_flags(), MS_RELATIME);
        assert!(parser.next().is_none());

        let content = b"335 294 0:56 / /proc rw,relatime shared:12 master:1 - proc proc rw";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 335);
        assert_eq!(mount_point.parent_id, 294);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 56);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, vec!(OsStr::new("shared:12"), OsStr::new("master:1")));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_flags(), MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_escaping() {
        let content = br"76 24 8:6 / /home/my\040super\134name rw,relatime shared:29 - ext4 /dev/sda1 rw,data=ordered";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 76);
        assert_eq!(mount_point.parent_id, 24);
        assert_eq!(mount_point.major, 8);
        assert_eq!(mount_point.minor, 6);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/home/my super\\name"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, vec!(OsStr::new("shared:29")));
        assert_eq!(mount_point.fstype, OsStr::new("ext4"));
        assert_eq!(mount_point.mount_source, OsStr::new("/dev/sda1"));
        assert_eq!(mount_point.super_options, OsStr::new("rw,data=ordered"));
        assert_eq!(mount_point.get_flags(), MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mounts_parser_non_utf8() {
        let content = b"22 24 0:19 / /\xff rw shared:5 - tmpfs tmpfs rw,mode=755";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new(OsStr::from_bytes(b"/\xff")));
        assert_eq!(mount_point.mount_options, OsStr::new("rw"));
        assert_eq!(mount_point.fstype, OsStr::new("tmpfs"));
        assert_eq!(mount_point.mount_source, OsStr::new("tmpfs"));
        assert_eq!(mount_point.get_flags(), 0);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mounts_parser_crlf() {
        let content = b"26 20 0:21 / /tmp rw shared:4 - tmpfs tmpfs rw\r\n\
                        \n\
                        27 22 0:22 / /tmp rw,nosuid,nodev shared:6 - tmpfs tmpfs rw\n";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new("/tmp"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_flags(), 0);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new("/tmp"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,nosuid,nodev"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_flags(), MS_NOSUID | MS_NODEV);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_incomplete_row() {
        let content = b"19 24 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError(..)) => {}
            _ => panic!("Expected incomplete row error")
        }
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_invalid_int() {
        let content = b"19 24b 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError(..)) => {}
            _ => panic!("Expected invalid row error")
        }
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_invalid_escape() {
        let content = b"19 24 0:4 / /proc\\01 rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let mut parser = MountInfoParser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError(..)) => {}
            _ => panic!("Expected invalid row error")
        }
        assert!(parser.next().is_none());
    }
}
