extern crate libmount;
extern crate argparse;
extern crate env_logger;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

use argparse::{ArgumentParser, Parse, StoreTrue};


fn main() {
    env_logger::init();
    let mut path = PathBuf::new();
    let mut bind = false;
    let mut readonly = false;
    let mut nodev = false;
    let mut noexec = false;
    let mut nosuid = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Remount utility. Similar to `mount -o remount` \
                            but keeps current mount point options");
        ap.refer(&mut path).add_argument("path", Parse,
            "Directory for remounting").required();
        ap.refer(&mut bind).add_option(&["--bind"], StoreTrue,
            "Set bind mount option");
        ap.refer(&mut readonly).add_option(&["--readonly"], StoreTrue,
            "Set readonly mount option");
        ap.refer(&mut nodev).add_option(&["--nodev"], StoreTrue,
            "Set nodev mount option");
        ap.refer(&mut noexec).add_option(&["--noexec"], StoreTrue,
            "Set noexec mount option");
        ap.refer(&mut nosuid).add_option(&["--nosuid"], StoreTrue,
            "Set nosuid mount option");
        ap.parse_args_or_exit();
    }
    match libmount::Remount::new(path)
        .bind(bind)
        .readonly(readonly)
        .nodev(nodev)
        .noexec(noexec)
        .nosuid(nosuid)
        .remount()
    {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(1);
        }
    }
}
