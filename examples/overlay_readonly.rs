extern crate libmount;
extern crate argparse;
extern crate env_logger;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

use argparse::{ArgumentParser, Parse, Collect};


fn main() {
    env_logger::init();
    let mut lowerdirs = Vec::<String>::new();
    let mut target = PathBuf::new();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Overlayfs mount utility.
                            Similar to `mount -t overlay`");
        ap.refer(&mut target)
            .add_argument("target", Parse,
            "The destination directory for mount").required();
        ap.refer(&mut lowerdirs).add_argument("lowerdir", Collect,
            "The source layers of the overlay").required();
        ap.parse_args_or_exit();
    }
    match libmount::Overlay::readonly(lowerdirs.iter().map(|x| x.as_ref()),
                                      target)
        .mount()
    {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(1);
        }
    }
}
