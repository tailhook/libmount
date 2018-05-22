extern crate libmount;
extern crate argparse;
extern crate env_logger;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

use argparse::{ArgumentParser, Parse};


fn main() {
    env_logger::init();
    let mut source = PathBuf::new();
    let mut target = PathBuf::new();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Move mountpoint utility. \
                            Similar to `mount --move`");
        ap.refer(&mut source).add_argument("source", Parse,
            "Source directory for bind mount").required();
        ap.refer(&mut target).add_argument("target", Parse,
            "Target directory for bind mount").required();
        ap.parse_args_or_exit();
    }
    match libmount::Move::new(source, target).move_mountpoint() {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(1);
        }
    }
}
