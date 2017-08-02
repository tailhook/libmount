use std::io::Read;
use std::fs::File;
use std::fmt::{Display, Debug};
use std::path::Path;

use nix::unistd::getuid;


pub trait Explainable: Display + Debug {
    fn explain(&self) -> String;
}

pub fn exists(path: &Path) -> &'static str {
    if path.exists() {
        "exists"
    } else {
        "missing"
    }
}

pub fn user() -> &'static str {
    let uid = getuid();
    if u32::from(uid) == 0 {
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
