extern crate libmount;
extern crate argparse;
extern crate env_logger;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

use argparse::{ArgumentParser, Parse, StoreOption};


fn main() {
    env_logger::init();
    let mut target = PathBuf::new();
    let mut size = None::<usize>;
    let mut mode = None::<String>;
    let mut uid = None::<u32>;
    let mut gid = None::<u32>;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Tmpfs mount utility. Similar to `mount --tmpfs`");
        ap.refer(&mut target).add_argument("target", Parse,
            "Target directory to mount tmpfs to").required();
        ap.refer(&mut size).add_option(&["--size"], StoreOption,
            "Set size of the filesystem");
        ap.refer(&mut mode).add_option(&["--mode"], StoreOption,
            "Set mode of the root directory");
        ap.refer(&mut uid).add_option(&["--uid"], StoreOption,
            "Set uid of the directory");
        ap.refer(&mut gid).add_option(&["--gid"], StoreOption,
            "Set gid of the directory");
        ap.parse_args_or_exit();
    }
    let mut mnt = libmount::Tmpfs::new(target);
    if let Some(x) = size { mnt = mnt.size_bytes(x); };
    if let Some(ref x) = mode {
        mnt = mnt.mode(u32::from_str_radix(x, 8).expect("valid octal mode"));
    }
    if let Some(x) = uid { mnt = mnt.uid(x); }
    if let Some(x) = gid { mnt = mnt.gid(x); }
    match mnt.mount() {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(1);
        }
    }
}
