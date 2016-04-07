use std::io::Read;
use std::fs::File;
use std::fmt::{Display, Debug};
use std::ffi::CString;

use nix::unistd::getuid;

use util::as_path;


pub trait Explainable: Display + Debug {
    fn explain(&self) -> String;
}

pub fn exists(path: &CString) -> &'static str {
    if as_path(path).exists() {
        "exists"
    } else {
        "missing"
    }
}

pub fn user() -> &'static str {
    let uid = getuid();
    if uid == 0 {
        let mut buf = String::with_capacity(100);
        match File::open("/proc/self/uid_map")
              .and_then(|mut f| f.read_to_string(&mut buf))
        {
            Ok(_) => {
                if buf == "         0          0 4294967295\n" {
                    "superuser"
                } else {
                    "mapped-root"
                }
            }
            Err(_) => {
                "privileged"
            }
        }
    } else {
        "regular-user"
    }
}
