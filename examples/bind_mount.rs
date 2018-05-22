extern crate libmount;
extern crate argparse;
extern crate env_logger;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

use argparse::{ArgumentParser, Parse, StoreFalse, StoreTrue};


fn main() {
    env_logger::init();
    let mut source = PathBuf::new();
    let mut target = PathBuf::new();
    let mut recursive = true;
    let mut readonly = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Bind mounting utility. Similar to `mount --bind`");
        ap.refer(&mut source).add_argument("source", Parse,
            "Source directory for bind mount").required();
        ap.refer(&mut target).add_argument("target", Parse,
            "Target directory for bind mount").required();
        ap.refer(&mut recursive).add_option(&["--non-recursive"], StoreFalse,
            "Disable recursive mount (only a real superuser can do this)");
        ap.refer(&mut readonly).add_option(&["--readonly"], StoreTrue,
            "Readonly mount");
        ap.parse_args_or_exit();
    }
    match libmount::BindMount::new(source, target)
        .recursive(recursive)
        .readonly(readonly)
        .mount()
    {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(1);
        }
    }
}
