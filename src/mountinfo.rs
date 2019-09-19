//! This module contains parser for /proc/PID/mountinfo
//!
use std;
use std::fmt;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::borrow::Cow;
use std::error::Error;

use nix::mount::MsFlags;

use libc::c_ulong;

/// Error parsing a single entry of mountinfo file
#[derive(Debug)]
pub(crate) struct ParseRowError(pub(crate) String);

impl fmt::Display for ParseRowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parse error: {}", self.0)
    }
}

impl Error for ParseRowError {
    fn description(&self) -> &str {
        return &self.0;
    }
}

/// Mountinfo file parsing error
#[derive(Debug)]
pub struct ParseError {
    msg: String,
    row_num: usize,
    row: String,
}

impl ParseError {
    fn new(msg: String, row_num: usize, row: String) -> ParseError {
        ParseError {
            msg: msg,
            row_num: row_num,
            row: row,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parse error at line {}: {}\n{}",
            self.row_num, self.description(), self.row)
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

/// A parser class for mountinfo file
#[derive(Debug)]
pub struct Parser<'a> {
    data: &'a [u8],
    row_num: usize,
    exhausted: bool,
}

#[allow(dead_code)]
impl<'a> Parser<'a> {
    /// Create a new parser
    ///
    /// `data` should contain whole contents of `mountinfo` file of any process
    pub fn new(data: &'a [u8]) -> Parser<'a> {
        Parser {
            data: data,
            row_num: 0,
            exhausted: false,
        }
    }
}

/// A single entry returned by mountpoint parser
#[allow(missing_docs)]  // self-descriptive / described by man page
#[derive(Debug)]
pub struct MountPoint<'a> {
    pub mount_id: c_ulong,
    pub parent_id: c_ulong,
    pub major: c_ulong,
    pub minor: c_ulong,
    pub root: Cow<'a, OsStr>,
    pub mount_point: Cow<'a, OsStr>,
    pub mount_options: Cow<'a, OsStr>,
    // TODO: we might need some enum which will have three states:
    // empty, single Cow<OsStr> value or a vector Vec<Cow<OsStr>>
    pub optional_fields: Cow<'a, OsStr>,
    pub fstype: Cow<'a, OsStr>,
    pub mount_source: Cow<'a, OsStr>,
    pub super_options: Cow<'a, OsStr>,
}

impl<'a> MountPoint<'a> {
    /// Returns flags of the mountpoint  as a numeric value
    ///
    /// This value matches linux `MsFlags::MS_*` flags as passed into mount syscall
    pub fn get_flags(&self) -> c_ulong {
        self.get_mount_flags().bits() as c_ulong
    }

    pub(crate) fn get_mount_flags(&self) -> MsFlags {
        let mut flags = MsFlags::empty();
        for opt in self.mount_options.as_bytes().split(|c| *c == b',') {
            let opt = OsStr::from_bytes(opt);
            if opt == OsStr::new("ro") { flags |= MsFlags::MS_RDONLY }
            else if opt == OsStr::new("nosuid") { flags |= MsFlags::MS_NOSUID }
            else if opt == OsStr::new("nodev") { flags |= MsFlags::MS_NODEV }
            else if opt == OsStr::new("noexec") { flags |= MsFlags::MS_NOEXEC }
            else if opt == OsStr::new("mand") { flags |= MsFlags::MS_MANDLOCK }
            else if opt == OsStr::new("sync") { flags |= MsFlags::MS_SYNCHRONOUS }
            else if opt == OsStr::new("dirsync") { flags |= MsFlags::MS_DIRSYNC }
            else if opt == OsStr::new("noatime") { flags |= MsFlags::MS_NOATIME }
            else if opt == OsStr::new("nodiratime") { flags |= MsFlags::MS_NODIRATIME }
            else if opt == OsStr::new("relatime") { flags |= MsFlags::MS_RELATIME }
            else if opt == OsStr::new("strictatime") { flags |= MsFlags::MS_STRICTATIME }
        }
        flags
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<MountPoint<'a>, ParseError>;

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
                    let res = match parse_mount_point(row) {
                        Ok(None) => continue,
                        Ok(Some(v)) => Ok(v),
                        Err(e) => Err(ParseError::new(e.0, self.row_num,
                            String::from_utf8_lossy(row).into_owned())),
                    };
                    return Some(res);
                },
                None => {
                    self.exhausted = true;
                    let res = match parse_mount_point(self.data) {
                        Ok(None) => return None,
                        Ok(Some(v)) => Ok(v),
                        Err(e) => Err(ParseError::new(e.0, self.row_num,
                            String::from_utf8_lossy(self.data).into_owned())),
                    };
                    return Some(res);
                },
            }
        }
    }
}

pub(crate) fn parse_mount_point<'a>(row: &'a [u8])
     -> Result<Option<MountPoint<'a>>, ParseRowError>
{
    let row = rstrip_cr(&row);
    if is_comment_line(row) {
        return Ok(None);
    }

    let (mount_id, row) = try!(parse_int(row));
    let (parent_id, row) = try!(parse_int(row));
    let (major, minor, row) = try!(parse_major_minor(row));
    let (root, row) = try!(parse_os_str(row));
    let (mount_point, row) = try!(parse_os_str(row));
    let (mount_options, row) = try!(parse_os_str(row));
    let (optional_fields, row) = try!(parse_optional(row));
    let (fstype, row) = try!(parse_os_str(row));
    let (mount_source, row) = try!(parse_os_str(row));
    let (super_options, _) = try!(parse_os_str(row));
    // TODO: should we ignore extra fields?
    Ok(Some(MountPoint {
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
    }))
}

fn is_comment_line(row: &[u8]) -> bool {
    if row.is_empty() {
        return true;
    }
    for c in row {
        if *c == b' ' || *c == b'\t' {
            continue;
        }
        if *c == b'#' {
            return true;
        }
        return false;
    }
    return false;
}

fn rstrip_cr(row: &[u8]) -> &[u8] {
    if let Some((&b'\r', tail)) = row.split_last() {
        tail
    } else {
        row
    }
}

fn parse_field<'a>(data: &'a [u8], delimit: &'a [u8])
    -> Result<(&'a [u8], &'a [u8]), ParseRowError>
{
    if data.is_empty() {
        return Err(ParseRowError(format!("Expected more fields")));
    }
    let data = lstrip_whitespaces(data);
    Ok(split_by(data, delimit))
}

fn parse_os_str<'a>(data: &'a [u8])
    -> Result<(Cow<'a, OsStr>, &'a [u8]), ParseRowError>
{
    let (field, tail) = try!(parse_field(data, b" "));
    Ok((unescape_octals(OsStr::from_bytes(field)), tail))
}

fn parse_int(data: &[u8])
    -> Result<(c_ulong, &[u8]), ParseRowError>
{
    let (field, tail) = try!(parse_field(data, b" "));
    let v = try!(std::str::from_utf8(field).map_err(|e| {
        ParseRowError(format!("Cannot parse integer {:?}: {}",
            String::from_utf8_lossy(field).into_owned(), e))}));

    let v = try!(c_ulong::from_str_radix(v, 10).map_err(|e| {
        ParseRowError(format!("Cannot parse integer {:?}: {}",
            String::from_utf8_lossy(field).into_owned(), e))}));
    Ok((v, tail))
}

fn parse_major_minor(data: &[u8])
    -> Result<(c_ulong, c_ulong, &[u8]), ParseRowError>
{
    let (major_field, data) = try!(parse_field(data, b":"));
    let (minor_field, tail) = try!(parse_field(data, b" "));
    let (major, _) = try!(parse_int(major_field));
    let (minor, _) = try!(parse_int(minor_field));
    Ok((major, minor, tail))
}

fn parse_optional<'a>(data: &'a [u8])
    -> Result<(Cow<'a, OsStr>, &'a [u8]), ParseRowError>
{
    let (field, tail) = try!(parse_field(data, b"- "));
    let field = rstrip_whitespaces(field);
    Ok((unescape_octals(OsStr::from_bytes(field)), tail))
}

fn lstrip_whitespaces(v: &[u8]) -> &[u8] {
    for (i, c) in v.iter().enumerate() {
        if *c != b' ' {
            return &v[i..];
        }
    }
    return &v[0..0];
}

fn rstrip_whitespaces(v: &[u8]) -> &[u8] {
    for (i, c) in v.iter().enumerate().rev() {
        if *c != b' ' {
            return &v[..i + 1];
        }
    }
    return &v[0..0];
}

fn split_by<'a, 'b>(v: &'a [u8], needle: &'b [u8]) -> (&'a [u8], &'a [u8]) {
    if needle.len() > v.len() {
        return (&v[0..], &v[0..0]);
    }
    let mut i = 0;
    while i <= v.len() - needle.len() {
        let (head, tail) = v.split_at(i);
        if tail.starts_with(needle) {
            return (head, &tail[needle.len()..]);
        }
        i += 1;
    }
    return (&v[0..], &v[0..0]);
}

fn unescape_octals(s: &OsStr) -> Cow<OsStr> {
    let (mut i, has_escapes) = {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if is_octal_encoding(&bytes[i..]) {
                break;
            }
            i += 1;
        }
        (i, i < bytes.len())
    };
    if !has_escapes {
        return Cow::Borrowed(s);
    }

    let mut v: Vec<u8> = vec!();
    let bytes = s.as_bytes();
    v.extend_from_slice(&bytes[..i]);
    while i < bytes.len() {
        if is_octal_encoding(&bytes[i..]) {
            let c = parse_octal(&bytes[i + 1..]);
            v.push(c);
            i += 4;
        } else {
            v.push(bytes[i]);
            i += 1;
        }
    }
    Cow::Owned(OsString::from_vec(v))
}

fn is_octal_encoding(v: &[u8]) -> bool {
    v.len() >= 4 && v[0] == b'\\'
        && is_oct(v[1]) && is_oct(v[2]) && is_oct(v[3])
}

fn is_oct(c: u8) -> bool {
    c >= b'0' && c <= b'7'
}

fn parse_octal(v: &[u8]) -> u8 {
    ((v[0] & 7) << 6) + ((v[1] & 7) << 3) + (v[2] & 7)
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    use nix::mount::MsFlags;

    use super::{Parser, ParseError};
    use super::{is_octal_encoding, parse_octal, unescape_octals};

    #[test]
    fn test_is_octal_encoding() {
        assert!(is_octal_encoding(b"\\000"));
        assert!(is_octal_encoding(b"\\123"));
        assert!(is_octal_encoding(b"\\777"));
        assert!(!is_octal_encoding(b""));
        assert!(!is_octal_encoding(b"\\"));
        assert!(!is_octal_encoding(b"000"));
        assert!(!is_octal_encoding(b"\\00"));
        assert!(!is_octal_encoding(b"\\800"));
    }

    #[test]
    fn test_parse_octal() {
        assert_eq!(parse_octal(b"000"), 0);
        assert_eq!(parse_octal(b"123"), 83);
        assert_eq!(parse_octal(b"377"), 255);
        // mount utility just ignores overflowing
        assert_eq!(parse_octal(b"777"), 255);
    }

    #[test]
    fn test_unescape_octals() {
        assert_eq!(unescape_octals(OsStr::new("\\000")), OsStr::from_bytes(b"\x00"));
        assert_eq!(unescape_octals(OsStr::new("\\00")), OsStr::new("\\00"));
        assert_eq!(unescape_octals(OsStr::new("test\\040data")), OsStr::new("test data"));
    }

    #[test]
    fn test_mount_info_parser_proc() {
        let content = b"19 24 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 19);
        assert_eq!(mount_point.parent_id, 24);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 4);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,nosuid,nodev,noexec,relatime"));
        assert_eq!(mount_point.optional_fields, OsStr::new("shared:12"));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_comment() {
        let content = b"# Test comment\n\
                        \t # Another shifted comment\n\
                        19 24 0:4 / /#proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 19);
        assert_eq!(mount_point.parent_id, 24);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 4);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/#proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,nosuid,nodev,noexec,relatime"));
        assert_eq!(mount_point.optional_fields, OsStr::new("shared:12"));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC | MsFlags::MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_missing_optional_fields() {
        let content = b"335 294 0:56 / /proc rw,relatime - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 335);
        assert_eq!(mount_point.parent_id, 294);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 56);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, OsStr::new(""));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_more_optional_fields() {
        let content = b"335 294 0:56 / /proc rw,relatime shared:12 master:1 - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 335);
        assert_eq!(mount_point.parent_id, 294);
        assert_eq!(mount_point.major, 0);
        assert_eq!(mount_point.minor, 56);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/proc"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, OsStr::new("shared:12 master:1"));
        assert_eq!(mount_point.fstype, OsStr::new("proc"));
        assert_eq!(mount_point.mount_source, OsStr::new("proc"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_escaping() {
        let content = br"76 24 8:6 / /home/my\040super\011name\012\134 rw,relatime shared:29 - ext4 /dev/sda1 rw,data=ordered";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_id, 76);
        assert_eq!(mount_point.parent_id, 24);
        assert_eq!(mount_point.major, 8);
        assert_eq!(mount_point.minor, 6);
        assert_eq!(mount_point.root, Path::new("/"));
        assert_eq!(mount_point.mount_point, Path::new("/home/my super\tname\n\\"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,relatime"));
        assert_eq!(mount_point.optional_fields, OsStr::new("shared:29"));
        assert_eq!(mount_point.fstype, OsStr::new("ext4"));
        assert_eq!(mount_point.mount_source, OsStr::new("/dev/sda1"));
        assert_eq!(mount_point.super_options, OsStr::new("rw,data=ordered"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_RELATIME);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_non_utf8() {
        let content = b"22 24 0:19 / /\xff rw shared:5 - tmpfs tmpfs rw,mode=755";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new(OsStr::from_bytes(b"/\xff")));
        assert_eq!(mount_point.mount_options, OsStr::new("rw"));
        assert_eq!(mount_point.fstype, OsStr::new("tmpfs"));
        assert_eq!(mount_point.mount_source, OsStr::new("tmpfs"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::empty());
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_crlf() {
        let content = b"26 20 0:21 / /tmp rw shared:4 - tmpfs tmpfs rw\r\n\
                        \n\
                        \r\n\
                        27 22 0:22 / /tmp rw,nosuid,nodev shared:6 - tmpfs tmpfs rw\r";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new("/tmp"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::empty());
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new("/tmp"));
        assert_eq!(mount_point.mount_options, OsStr::new("rw,nosuid,nodev"));
        assert_eq!(mount_point.super_options, OsStr::new("rw"));
        assert_eq!(mount_point.get_mount_flags(), MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_incomplete_row() {
        let content = b"19 24 0:4 / /proc rw,relatime shared:12 - proc proc";
        let mut parser = Parser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError {ref msg, ..}) => {
                assert_eq!(msg, "Expected more fields");
            },
            _ => panic!("Expected incomplete row error")
        }
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_invalid_int() {
        let content = b"19 24b 0:4 / /proc rw,relatime - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError {ref msg, ..}) => {
                assert!(msg.starts_with("Cannot parse integer \"24b\":"));
            },
            _ => panic!("Expected invalid row error")
        }
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_overflowed_int() {
        let content = b"111111111111111111111";
        let mut parser = Parser::new(&content[..]);
        let mount_info_res = parser.next().unwrap();
        assert!(mount_info_res.is_err());
        match mount_info_res {
            Err(ParseError {ref msg, ..}) => {
                assert!(msg.starts_with("Cannot parse integer \"111111111111111111111\""));
            },
            _ => panic!("Expected invalid row error")
        }
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_invalid_escape() {
        let content = b"19 24 0:4 / /proc\\1 rw,relatime - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new("/proc\\1"));
        assert!(parser.next().is_none());
    }

    #[test]
    fn test_mount_info_parser_overflowed_escape() {
        let content = b"19 24 0:4 / /proc\\400 rw,nosuid,nodev,noexec,relatime - proc proc rw";
        let mut parser = Parser::new(&content[..]);
        let mount_point = parser.next().unwrap().unwrap();
        assert_eq!(mount_point.mount_point, Path::new(OsStr::from_bytes(b"/proc\x00")));
        assert!(parser.next().is_none());
    }
}
