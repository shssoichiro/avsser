#![feature(plugin)]
#![plugin(clippy)]

extern crate avsser;
extern crate getopts;

use getopts::Options;
use std::env;
use std::path::PathBuf;
use avsser::generic::input::get_list_of_files;
use avsser::generic::output::create_avs_script;
use avsser::generic::output::AvsOptions;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] INPUT", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let program = env::args().next().unwrap();
    let args: Vec<String> = env::args().skip(1).collect();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let input = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(&program, opts);
        return;
    };

    let input = get_list_of_files(PathBuf::from(input), false).ok().expect("Unable to read input file(s)");
    for file in input {
        let path = PathBuf::from(file);
        create_avs_script(
            path.clone(),
            path.with_extension("avs"),
            AvsOptions {
                remove_grain: Some(1)
            }).ok();
    }
}
