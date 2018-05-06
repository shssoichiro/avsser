extern crate avsser;
extern crate getopts;

use getopts::Options;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use avsser::generic::input::get_list_of_files;
use avsser::generic::input::determine_input_type;
use avsser::generic::output::create_avs_script;
use avsser::generic::output::AvsOptions;
use avsser::generic::output::extract_fonts;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] INPUT", program);
    print!("{}", opts.usage(&brief));
}

fn resize_opt_into_dimensions(pair: &str) -> (u32, u32) {
    let items: Vec<&str> = pair.split(',').collect();
    if items.len() != 2 {
        panic!("Expected exactly 2 arguments (comma-separated) for 'resize'");
    }

    (
        items[0].parse().expect("Invalid width supplied to resizer"),
        items[1]
            .parse()
            .expect("Invalid height supplied to resizer"),
    )
}

fn main() {
    let program = env::args().next().unwrap();
    let args: Vec<String> = env::args().skip(1).collect();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optflag(
        "s",
        "subtitle",
        "include subtitles with TextSub(input_filename.ass)",
    );
    opts.optflag(
        "S",
        "sub-extract",
        "extract subtitles from the input files (defaults to track 0)",
    );
    opts.optopt(
        "T",
        "sub-track",
        "select which subtitle track to extract, 0-indexed (does nothing without -S)",
        "TRACK",
    );
    opts.optflag("a", "audio", "include audio from video");
    opts.optopt(
        "A",
        "audio-ext",
        "include audio from separate file with extension (takes precedence over audio \
         from video)",
        "EXT",
    );
    opts.optflag("f", "fonts", "extract fonts from mkv container");
    opts.optopt(
        "R",
        "resize",
        "resize video to the given width and height",
        "w,h",
    );
    opts.optflag("G", "keep-grain", "don't add a RemoveGrain(1) filter");
    opts.optflag(
        "",
        "120",
        "convert VFR to 120fps CFR (only works with MKVs)",
    );
    opts.optflag("", "10", "decode Hi10p video");

    let matches = match opts.parse(&args) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let input = if matches.free.is_empty() {
        print_usage(&program, opts);
        return;
    } else {
        matches.free[0].clone()
    };

    let input = get_list_of_files(Path::new(&input), false).unwrap();
    for file in input {
        if determine_input_type(file.as_ref()).is_none() {
            continue;
        }
        let path = PathBuf::from(file);
        if matches.opt_present("f") {
            extract_fonts(path.as_ref()).unwrap();
        }
        create_avs_script(
            path.as_ref(),
            path.with_extension("avs").as_ref(),
            &AvsOptions {
                remove_grain: if matches.opt_present("G") {
                    None
                } else {
                    Some(1)
                },
                ass: matches.opt_present("s"),
                ass_extract: if matches.opt_present("S") {
                    if let Some(track) = matches.opt_str("T") {
                        Some(track.parse().expect("No argument supplied for track"))
                    } else {
                        Some(0)
                    }
                } else {
                    None
                },
                audio: (matches.opt_present("a"), matches.opt_str("A")),
                resize: if matches.opt_present("R") {
                    Some(resize_opt_into_dimensions(
                        matches
                            .opt_str("R")
                            .expect("No argument supplied for resize")
                            .as_ref(),
                    ))
                } else {
                    None
                },
                to_cfr: matches.opt_present("120"),
                hi10p: matches.opt_present("10"),
            },
        ).unwrap();
    }
}
