use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use super::input::InputTypes;

pub struct AvsOptions {
    pub remove_grain: Option<u8>,
    pub ass: bool,
    pub ass_extract: Option<u8>,
    pub audio: (bool, Option<String>),
    pub resize: Option<(u32, u32)>,
}

pub fn create_avs_script(in_file: &Path, out_file: &Path, opts: AvsOptions) -> Result<(), String> {
    let breakpoints = match super::super::parsers::mkvinfo::get_ordered_chapters_list(in_file) {
        Ok(x) => x,
        Err(x) => return Err(x.to_owned()),
    };
    let mut iter = 0usize;
    let mut current_breakpoint = None;
    let mut current_filename = in_file.to_owned();
    let mut segments: Vec<String> = Vec::new();
    let mut cached_uuids: HashMap<[u8; 16], PathBuf> = HashMap::new();

    loop {
        if breakpoints.is_some() {
            current_breakpoint = breakpoints.clone().unwrap().get(iter).cloned();
            iter += 1;
            if current_breakpoint.is_none() {
                break;
            }
        }
        if current_breakpoint.is_some() &&
           current_breakpoint.clone().unwrap().foreign_uuid.is_some() {
            let current_uuid = current_breakpoint.clone().unwrap().foreign_uuid.unwrap();
            if let Some(filename) = cached_uuids.clone().get(&current_uuid) {
                current_filename = filename.to_owned();
            } else {
                for external in in_file.parent().unwrap().read_dir().unwrap() {
                    let path = external.unwrap().path();
                    if path.extension().unwrap() != "mkv" || path.as_path() == in_file {
                        continue;
                    }
                    if let Ok(uuid) = super::super::parsers::mkvinfo::get_file_uuid(&path) {
                        cached_uuids.insert(uuid, path.to_owned());
                        if uuid == current_uuid {
                            current_filename = path.to_owned();
                            break;
                        }
                    }
                }
            }
            if current_filename.as_path() == in_file {
                return Err("Could not find file linked through ordered chapters.".to_owned());
            }
        } else {
            current_filename = in_file.to_owned();
        }
        let mut current_string = "".to_owned();
        let video_filter = determine_video_source_filter(&current_filename);
        match opts.audio.clone() {
            (false, None) => {
                current_string.push_str(format!("{}(\"{}\")",
                                                video_filter,
                                                current_filename.file_name()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap())
                    .as_ref())
            }
            (true, None) => {
                current_string.push_str(format!("AudioDub({}(\"{}\"), FFAudioSource(\"{}\"))",
                                                video_filter,
                                                current_filename.file_name()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap(),
                                                current_filename.file_name()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap())
                    .as_ref())
            }
            (_, Some(x)) => {
                current_string.push_str(format!("AudioDub({}(\"{}\"), FFAudioSource(\"{}\"))",
                                                video_filter,
                                                current_filename.file_name()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap(),
                                                current_filename.with_extension(x)
                                                    .file_name()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap())
                    .as_ref())
            }
        }
        if let Some((width, height)) = opts.resize {
            current_string.push_str(format!(".Lanczos4Resize({}, {})", width, height).as_ref());
        }
        if let Some(remove_grain) = opts.remove_grain {
            current_string.push_str(format!(".RemoveGrain({})", remove_grain).as_ref());
        }
        if let Some(sub_track) = opts.ass_extract {
            if current_filename.with_extension("ass").exists() {
                println!("Cowardly refusing to overwrite existing subtitles.");
            } else {
                try!(extract_subtitles(current_filename.as_ref(), sub_track));
            }
        }
        if opts.ass {
            current_string.push_str(format!(".TextSub(\"{}\")",
                                            current_filename.with_extension("ass")
                                                .file_name()
                                                .unwrap()
                                                .to_str()
                                                .unwrap())
                .as_ref());
        }
        if breakpoints.is_some() {
            current_string.push_str(format!(".Trim({},{})",
                                            current_breakpoint.clone().unwrap().start_frame,
                                            current_breakpoint.clone().unwrap().end_frame)
                .as_ref());
            segments.push(current_string);
        } else {
            segments.push(current_string);
            break;
        }
    }

    let mut script = match File::create(out_file) {
        Ok(x) => x,
        Err(x) => return Err(format!("{}", x)),
    };

    match writeln!(&mut script, "{}", segments.join("\\\n++ ")) {
        Ok(_) => Ok(()),
        Err(x) => Err(format!("{}", x)),
    }
}

pub fn determine_video_source_filter(path: &Path) -> String {
    match super::input::determine_input_type(path) {
        Some(InputTypes::DgIndex) => "DGDecode_MPEG2Source".to_owned(),
        Some(InputTypes::DgAvc) => "AVCSource".to_owned(),
        Some(_) => "FFVideoSource".to_owned(),
        None => panic!("Invalid input type"),
    }
}

pub fn extract_subtitles(in_file: &Path, sub_track: u8) -> Result<(), String> {
    match Command::new("ffmpeg")
        .args(&["-i",
                in_file.to_str().unwrap().as_ref(),
                "-map",
                &format!("0:s:{}", sub_track),
                "-map_chapters",
                "-1",
                in_file.with_extension("ass").to_str().unwrap().as_ref()])
        .status() {
        Ok(_) => Ok(()),
        Err(x) => Err(format!("{}", x)),
    }
}

pub fn extract_fonts(in_file: &Path) -> Result<(), String> {
    let fonts = match super::super::parsers::mkvinfo::get_fonts_list(in_file) {
        Ok(x) => x,
        Err(x) => return Err(x.to_owned()),
    };
    for (id, filename) in &fonts {
        let font_path = in_file.with_file_name(filename);
        if !font_path.exists() {
            match Command::new("mkvextract")
                .args(&["attachments",
                        in_file.to_str().unwrap().as_ref(),
                        format!("{}:{}", id, font_path.to_str().unwrap()).as_ref()])
                .status() {
                Ok(_) => (),
                Err(x) => return Err(format!("{}", x)),
            };
        }
    }

    Ok(())
}
