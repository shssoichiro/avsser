#![warn(clippy::all)]

use avsser::input::determine_input_type;
use avsser::input::get_list_of_files;
use avsser::output::*;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use std::path::Path;

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("input").help("the input file to generate a script for").required(true).index(1))
        .arg(Arg::with_name("subtitle")
            .short("s")
            .long("subtitle")
            .help("include subtitles with TextSub(input_filename.ass)"))
        .arg(Arg::with_name("sub-extract").short("S").long("sub-extract").help("extract subtitles from the input files (defaults to track 0)"))
        .arg(Arg::with_name("sub-track").short("T").long("sub-track").help("select which subtitle track to extract, 0-indexed (does nothing without -S)")
            .takes_value(true).value_name("TRACK"))
        .arg(Arg::with_name("audio").short("a").long("audio").help("include audio from video"))
        .arg(Arg::with_name("audio-ext").short("A").long("audio-ext").help("include audio from separate file with specified extension (takes precedence over audio \
         from video)").takes_value(true).value_name("EXT"))
        .arg(Arg::with_name("fonts").short("f").long("fonts").help("extract fonts from mkv container"))
        .arg(Arg::with_name("resize").short("R").long("resize").help("resize video to the given width and height").takes_value(true).value_name("W,H"))
        .arg(Arg::with_name("filters").short("F").long("filters").help("use a custom filter chain instead of RemoveGrain(1)"))
        .arg(Arg::with_name("keep-grain").short("G").long("keep-grain").help("don't add a RemoveGrain(1) filter"))
        .arg(Arg::with_name("120").long("120").help("convert VFR to 120fps CFR (only works with MKVs)"))
        .arg(Arg::with_name("downsample").long("downsample").alias("ds").help("downsample video to YUV420P8"))
        .arg(Arg::with_name("vapour").long("vs").help("generate a vapoursynth script instead"))
        .get_matches();

    let input = matches.value_of("input").unwrap();
    let input = get_list_of_files(Path::new(&input), false).unwrap();
    for path in input {
        if determine_input_type(path.as_ref()).is_none() {
            continue;
        }
        if matches.is_present("fonts") {
            extract_fonts(path.as_ref()).unwrap();
        }
        create_output(&path, &matches).unwrap();
    }
}

fn resize_opt_into_dimensions(pair: &str) -> (u32, u32) {
    let items: Vec<&str> = pair.split(|c| c == ',' || c == 'x' || c == 'X').collect();
    if items.len() != 2 {
        panic!("Expected exactly 2 arguments (comma-separated or x-separated) for 'resize'");
    }

    (
        items[0].parse().expect("Invalid width supplied to resizer"),
        items[1]
            .parse()
            .expect("Invalid height supplied to resizer"),
    )
}

fn create_output(path: &Path, matches: &ArgMatches) -> Result<(), String> {
    let opts = AvsOptions {
        filters: if matches.is_present("filters") {
            // FIXME: This is probably broken with avs, definitely broken with vpy
            vec![matches
                .value_of("filters")
                .unwrap()
                .trim_start_matches('.')
                .to_string()]
        } else {
            vec![]
        },
        ass: matches.is_present("subtitle"),
        ass_extract: if matches.is_present("sub-extract") {
            Some(
                matches
                    .value_of("sub-track")
                    .map(|track| track.parse().expect("Invalid argument supplied for track"))
                    .unwrap_or(0),
            )
        } else {
            None
        },
        audio: (
            matches.is_present("audio"),
            matches.value_of("audio-ext").map(|ext| ext.to_string()),
        ),
        resize: matches
            .value_of("resize")
            .map(|resize| resize_opt_into_dimensions(resize)),
        to_cfr: matches.is_present("120"),
        downsample: matches.is_present("downsample"),
    };
    let writer: Box<dyn ScriptFormat> = if matches.is_present("vapour") {
        Box::new(VapoursynthWriter::new(
            opts,
            !matches.is_present("keep-grain"),
        ))
    } else {
        Box::new(AvisynthWriter::new(opts, !matches.is_present("keep-grain")))
    };
    writer.create_script(path, &path.with_extension(writer.get_script_extension()))
}
