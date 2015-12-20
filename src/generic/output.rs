use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub struct AvsOptions {
    pub remove_grain: Option<u8>,
    pub ass: bool,
    pub ass_extract: bool,
    pub audio: (bool, Option<String>)
}

pub fn create_avs_script(in_file: &Path, out_file: &Path, opts: AvsOptions) -> Result<(), String> {
    let mut script = match File::create(out_file) {
        Ok(x) => x,
        Err(x) => return Err(format!("{}", x))
    };

    match opts.audio {
        (false, None) => writeln!(&mut script, "FFVideoSource(\"{}\")", in_file.to_str().unwrap()).unwrap(),
        (true, None) => writeln!(&mut script, "AudioDub(FFVideoSource(\"{}\"), FFAudioSource(\"{}\"))", in_file.to_str().unwrap(), in_file.to_str().unwrap()).unwrap(),
        (_, Some(x)) => writeln!(&mut script, "AudioDub(FFVideoSource(\"{}\"), FFAudioSource(\"{}\"))", in_file.to_str().unwrap(), in_file.with_extension(x).to_str().unwrap()).unwrap(),
    }
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

pub fn extract_subtitles(in_file: &Path) -> Result<(), String> {
    match Command::new("ffmpeg")
        .args(&["-i",
            format!("{}", in_file.to_str().unwrap()).as_ref(),
            "-an",
            "-vn",
            "-c:s:0",
            "copy",
            "-map_chapters",
            "-1",
            format!("{}", in_file.with_extension("ass").to_str().unwrap()).as_ref()])
        .status() {
            Ok(_) => Ok(()),
            Err(x) => Err(format!("{}", x))
        }
}

pub fn extract_fonts(in_file: &Path) -> Result<(), String> {
    let fonts = match super::super::parsers::mkvinfo::get_fonts_list(in_file) {
        Ok(x) => x,
        Err(x) => return Err(format!("{}", x))
    };
    for (id, filename) in &fonts {
        let font_path = in_file.with_file_name(filename);
        if !font_path.exists() {
            match Command::new("mkvextract")
                .args(&["attachments",
                    format!("{}", in_file.to_str().unwrap()).as_ref(),
                    format!("{}:{}", id, font_path.to_str().unwrap()).as_ref()])
                .status() {
                    Ok(_) => (),
                    Err(x) => return Err(format!("{}", x))
                };
        }
    }

    Ok(())
}
