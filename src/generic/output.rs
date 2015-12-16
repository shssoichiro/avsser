use std::fs::File;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

pub struct AvsOptions {
    pub remove_grain: Option<u8>,
    pub ass: bool,
    pub ass_extract: bool
}

pub fn create_avs_script(in_file: PathBuf, out_file: PathBuf, opts: AvsOptions) -> Result<(), Box<Error>> {
    let mut script = try!(File::create(out_file));
    writeln!(&mut script, "FFVideoSource(\"{}\")", in_file.to_str().unwrap()).unwrap();
    if let Some(remove_grain) = opts.remove_grain {
        writeln!(&mut script, "RemoveGrain({})", remove_grain).unwrap();
    }
    if opts.ass_extract {
        try!(extract_subtitles(in_file.clone()));
    }
    if opts.ass {
        writeln!(&mut script, "TextSub(\"{}\")", in_file.with_extension("ass").to_str().unwrap()).unwrap();
    }

    Ok(())
}

pub fn extract_subtitles(in_file: PathBuf) -> Result<(), Box<Error>> {
    try!(Command::new("ffmpeg")
        .args(&["-i",
            format!("{}", in_file.to_str().unwrap()).as_ref(),
            "-an",
            "-vn",
            "-c:s:0",
            "copy",
            "-map_chapters",
            "-1",
            format!("{}", in_file.with_extension("ass").to_str().unwrap()).as_ref()])
        .status());

    Ok(())
}
