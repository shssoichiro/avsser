use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;
use super::input::InputTypes;

pub struct AvsOptions {
    pub remove_grain: Option<u8>,
    pub ass: bool,
    pub ass_extract: Option<u8>,
    pub audio: (bool, Option<String>),
    pub resize: Option<(u32, u32)>,
    pub to_cfr: bool,
    pub hi10p: bool,
}

pub fn create_avs_script(in_file: &Path, out_file: &Path, opts: &AvsOptions) -> Result<(), String> {
    let breakpoints = match super::super::parsers::mkvinfo::get_ordered_chapters_list(in_file) {
        Ok(x) => x,
        Err(x) => return Err(x.to_owned()),
    };
    let mut iter = 0usize;
    let mut current_breakpoint = None;
    let mut segments: Vec<String> = Vec::new();
    let mut cached_uuids: HashMap<Uuid, PathBuf> = HashMap::new();

    loop {
        if breakpoints.is_some() {
            current_breakpoint = breakpoints.clone().unwrap().get(iter).cloned();
            iter += 1;
            if current_breakpoint.is_none() {
                break;
            }
        }

        let mut current_filename = in_file.to_owned();
        if let Some(ref current_breakpoint) = current_breakpoint {
            if let Some(current_uuid) = current_breakpoint.foreign_uuid {
                if let Some(filename) = cached_uuids.get(&current_uuid).cloned() {
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
            }
        }

        let mut current_string = String::new();
        let video_filter = if opts.hi10p {
            "LWLibAvVideoSource"
        } else {
            determine_video_source_filter(&current_filename)
        };
        let timecodes_path = current_filename.with_extension("timecodes.txt");
        if opts.to_cfr && !timecodes_path.exists() {
            File::create(timecodes_path).ok();
        }
        let mut video_filter_str =
            format!("{}(\"{}\"{})",
                    video_filter,
                    current_filename.canonicalize().unwrap().to_str().unwrap(),
                    if opts.to_cfr {
                        format!(", timecodes=\"{}\"",
                                current_filename
                                    .with_extension("timecodes.txt")
                                    .canonicalize()
                                    .unwrap()
                                    .to_str()
                                    .unwrap())
                    } else {
                        String::new()
                    });
        if opts.hi10p {
            video_filter_str.push_str(".f3kdb(input_depth=10, input_mode=2, output_depth=8)");
        }
        if opts.to_cfr {
            // This needs to happen before the `AudioDub`
            // Also, `vfrtocfr` requires the full path to the timecodes file
            video_filter_str
                .push_str(format!(".vfrtocfr(timecodes=\"{}\", fpsnum=120000, fpsden=1001)",
                                  current_filename
                                      .with_extension("timecodes.txt")
                                      .canonicalize()
                                      .unwrap()
                                      .to_str()
                                      .unwrap())
                                  .as_ref());
        }
        match opts.audio {
            (false, None) => current_string.push_str(&video_filter_str),
            (true, None) => {
                current_string.push_str(format!("AudioDub({}, FFAudioSource(\"{}\"))",
                                                video_filter_str,
                                                current_filename
                                                    .canonicalize()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap())
                                                .as_ref())
            }
            (_, Some(ref x)) => {
                current_string.push_str(format!("AudioDub({}, FFAudioSource(\"{}\"))",
                                                video_filter_str,
                                                current_filename
                                                    .with_extension(x)
                                                    .canonicalize()
                                                    .unwrap()
                                                    .to_str()
                                                    .unwrap())
                                                .as_ref())
            }
        }
        if let Some((width, height)) = opts.resize {
            current_string.push_str(format!(".Spline64Resize({}, {})", width, height).as_ref());
        }
        if let Some(remove_grain) = opts.remove_grain {
            current_string.push_str(format!(".RemoveGrain({})", remove_grain).as_ref());
        }
        if let Some(sub_track) = opts.ass_extract {
            if current_filename.with_extension("ass").exists() {
                println!(
                    "Cowardly refusing to overwrite existing subtitles: {}",
                    current_filename.with_extension("ass").to_string_lossy()
                );
            } else {
                extract_subtitles(current_filename.as_ref(), sub_track)?;
            }
        }
        if opts.ass {
            current_string.push_str(format!(".TextSub(\"{}\")",
                                            current_filename
                                                .with_extension("ass")
                                                .canonicalize()
                                                .unwrap()
                                                .to_str()
                                                .unwrap())
                                            .as_ref());
        }
        if breakpoints.is_some() {
            current_string.push_str(
                format!(
                    ".Trim({},{})",
                    current_breakpoint.clone().unwrap().start_frame,
                    current_breakpoint.clone().unwrap().end_frame
                ).as_ref(),
            );
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

pub fn determine_video_source_filter(path: &Path) -> &'static str {
    match super::input::determine_input_type(path) {
        Some(InputTypes::DgIndex) => "DGDecode_MPEG2Source",
        Some(InputTypes::DgAvc) => "AVCSource",
        Some(_) => "FFVideoSource",
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
                .args(&[
                    "attachments",
                    in_file.to_str().unwrap().as_ref(),
                    format!("{}:{}", id, font_path.to_str().unwrap()).as_ref(),
                ])
                .status()
            {
                Ok(_) => (),
                Err(x) => return Err(format!("{}", x)),
            };
        }
    }

    Ok(())
}
