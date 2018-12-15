use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use uuid::Uuid;

use crate::generic::input::determine_input_type;
use crate::parsers::mkvinfo::get_file_uuid;
use crate::parsers::mkvinfo::get_fonts_list;
use crate::parsers::mkvinfo::get_ordered_chapters_list;
use crate::parsers::mkvinfo::BreakPoint;

use super::input::InputTypes;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputType {
    Avisynth,
    Vapoursynth,
}

#[derive(Debug, Clone)]
pub struct AvsOptions {
    pub script_type: OutputType,
    pub filters: Vec<String>,
    pub ass: bool,
    pub ass_extract: Option<u8>,
    pub audio: (bool, Option<String>),
    pub resize: Option<(u32, u32)>,
    pub to_cfr: bool,
    pub hi10p: bool,
}

pub fn create_script(in_file: &Path, out_file: &Path, opts: &AvsOptions) -> Result<(), String> {
    let breakpoints = get_ordered_chapters_list(in_file, opts.to_cfr)?;
    let mut iter = 0usize;
    let mut current_breakpoint = None;
    let mut segments: Vec<Vec<String>> = Vec::new();
    let mut cached_uuids: HashMap<Uuid, PathBuf> = HashMap::new();
    let mut preloads: HashMap<PathBuf, String> = HashMap::new();

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
                        if let Ok(uuid) = get_file_uuid(&path) {
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

        let mut current_filters = Vec::new();
        if opts.to_cfr && !preloads.contains_key(&current_filename) {
            preloads.insert(
                current_filename.clone(),
                build_video_filter_string(&current_filename, opts, true),
            );
        }
        current_filters.push(build_video_filter_string(&current_filename, opts, false));
        if opts.to_cfr {
            // This needs to happen before the `AudioDub`
            // Also, `vfrtocfr` requires the full path to the timecodes file
            current_filters.push(build_vfr_string(
                &current_filename.with_extension("timecodes.txt"),
                opts,
            ));
        }
        match opts.audio {
            (false, None) => (),
            (true, None) => {
                current_filters.push(build_audio_dub_string(
                    &current_filename.canonicalize().unwrap(),
                    opts,
                ));
            }
            (_, Some(ref x)) => {
                current_filters.push(build_audio_dub_string(
                    &current_filename.with_extension(x).canonicalize().unwrap(),
                    opts,
                ));
            }
        }
        if !opts.filters.is_empty() {
            current_filters.extend_from_slice(&opts.filters);
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
            current_filters.push(build_subtitle_string(&current_filename, opts));
        }
        if let Some((width, height)) = opts.resize {
            current_filters.push(build_resize_string(width, height, opts));
        }
        if breakpoints.is_some() {
            current_filters.push(build_trim_string(current_breakpoint.unwrap(), opts));
            segments.push(current_filters);
        } else {
            segments.push(current_filters);
            break;
        }
    }

    let mut script = match File::create(out_file) {
        Ok(x) => x,
        Err(x) => return Err(format!("{}", x)),
    };

    if opts.script_type == OutputType::Vapoursynth {
        writeln!(&mut script, "from vapoursynth import core").map_err(|e| e.to_string())?;
        writeln!(&mut script).map_err(|e| e.to_string())?;
    }

    if !preloads.is_empty() {
        writeln!(
            &mut script,
            "{}",
            preloads
                .values()
                .cloned()
                .collect::<Vec<String>>()
                .join("\n")
        )
        .map_err(|e| e.to_string())?;
        writeln!(&mut script).map_err(|e| e.to_string())?;
    }

    write_segments(&segments, opts.script_type, &mut script)
}

#[inline]
pub fn determine_video_source_filter(path: &Path, opts: &AvsOptions) -> &'static str {
    match opts.script_type {
        OutputType::Avisynth => match determine_input_type(path) {
            Some(InputTypes::DgIndex) => "DGDecode_MPEG2Source",
            Some(InputTypes::DgAvc) => "AVCSource",
            Some(_) => "FFVideoSource",
            None => panic!("Invalid input type"),
        },
        OutputType::Vapoursynth => match determine_input_type(path) {
            Some(InputTypes::DgIndex) => "core.d2v.Source",
            Some(InputTypes::DgAvc) => unimplemented!(),
            Some(_) => "core.ffms2.Source",
            None => panic!("Invalid input type"),
        },
    }
}

#[inline]
pub fn get_default_filters(output: OutputType) -> &'static str {
    match output {
        OutputType::Avisynth => "RemoveGrain(1)",
        OutputType::Vapoursynth => "core.rgvs.RemoveGrain(1)",
    }
}

pub fn extract_subtitles(in_file: &Path, sub_track: u8) -> Result<(), String> {
    match Command::new("ffmpeg")
        .args(&[
            "-i",
            in_file.to_str().unwrap(),
            "-map",
            &format!("0:s:{}", sub_track),
            "-map_chapters",
            "-1",
            in_file.with_extension("ass").to_str().unwrap(),
        ])
        .status()
    {
        Ok(_) => Ok(()),
        Err(x) => Err(format!("{}", x)),
    }
}

pub fn extract_fonts(in_file: &Path) -> Result<(), String> {
    let fonts = match get_fonts_list(in_file) {
        Ok(x) => x,
        Err(x) => return Err(x.to_owned()),
    };
    for (id, filename) in &fonts {
        let font_path = in_file.with_file_name(filename);
        if !font_path.exists() {
            match Command::new("mkvextract")
                .args(&[
                    "attachments",
                    in_file.to_str().unwrap(),
                    &format!("{}:{}", id, font_path.to_str().unwrap()),
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

fn build_video_filter_string(
    current_filename: &Path,
    opts: &AvsOptions,
    is_preload: bool,
) -> String {
    let video_filter = get_video_filter_full_name(&current_filename, opts);
    let timecodes_path = current_filename.with_extension("timecodes.txt");
    if opts.to_cfr && !timecodes_path.exists() {
        File::create(&timecodes_path).ok();
    }
    let mut filter_opts = String::new();
    if opts.hi10p {
        filter_opts.push_str(", format = \"YUV420P8\"");
    }
    if opts.to_cfr && is_preload {
        filter_opts.push_str(&format!(
            ", timecodes=\"{}\"",
            match opts.script_type {
                OutputType::Avisynth => timecodes_path
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                OutputType::Vapoursynth => timecodes_path
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(r"\", r"\\"),
            },
        ));
    }

    format!(
        "{}({}{})",
        video_filter,
        match opts.script_type {
            OutputType::Avisynth => format!(
                "\"{}\"",
                current_filename.canonicalize().unwrap().to_str().unwrap()
            ),
            OutputType::Vapoursynth => format!(
                "source='{}'",
                current_filename
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(r"\", r"\\")
            ),
        },
        filter_opts
    )
}

fn build_vfr_string(timecodes_path: &Path, opts: &AvsOptions) -> String {
    match opts.script_type {
        OutputType::Avisynth => format!(
            "vfrtocfr(timecodes=\"{}\", fpsnum=120000, fpsden=1001)",
            timecodes_path.canonicalize().unwrap().to_str().unwrap()
        ),
        OutputType::Vapoursynth => format!(
            "vfrtocfr.VFRToCFR(\"{}\", 120000, 1001)",
            timecodes_path
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(r"\", r"\\")
        ),
    }
}

fn build_audio_dub_string(audio_filename: &Path, opts: &AvsOptions) -> String {
    match opts.script_type {
        OutputType::Avisynth => format!(
            "AudioDub(FFAudioSource(\"{}\"))",
            audio_filename.to_str().unwrap()
        ),
        OutputType::Vapoursynth => unimplemented!(),
    }
}

fn build_subtitle_string(current_filename: &Path, opts: &AvsOptions) -> String {
    let avs_file = current_filename
        .with_extension("ass")
        .canonicalize()
        .unwrap();
    match opts.script_type {
        OutputType::Avisynth => format!("TextSub(\"{}\")", avs_file.to_str().unwrap()),
        OutputType::Vapoursynth => format!(
            "core.xyvsf.TextSub(\'{}\')",
            avs_file.to_str().unwrap().replace(r"\", r"\\")
        ),
    }
}

fn build_resize_string(width: u32, height: u32, opts: &AvsOptions) -> String {
    match opts.script_type {
        OutputType::Avisynth => format!("Spline64Resize({}, {})", width, height),
        OutputType::Vapoursynth => format!("core.resize.Spline64({}, {})", width, height),
    }
}

fn build_trim_string(breakpoint: BreakPoint, opts: &AvsOptions) -> String {
    match opts.script_type {
        OutputType::Avisynth => {
            format!("Trim({},{})", breakpoint.start_frame, breakpoint.end_frame)
        }
        OutputType::Vapoursynth => format!(
            "core.std.Trim({}, {})",
            breakpoint.start_frame, breakpoint.end_frame
        ),
    }
}

fn get_video_filter_full_name(current_filename: &Path, opts: &AvsOptions) -> &'static str {
    match opts.script_type {
        OutputType::Avisynth => {
            if opts.hi10p {
                "LWLibAvVideoSource"
            } else {
                determine_video_source_filter(&current_filename, opts)
            }
        }
        OutputType::Vapoursynth => {
            if opts.hi10p {
                "core.lsmas.LWLibavSource"
            } else {
                determine_video_source_filter(&current_filename, opts)
            }
        }
    }
}

fn write_segments<W: Write>(
    segments: &[Vec<String>],
    output_type: OutputType,
    script: &mut W,
) -> Result<(), String> {
    for (i, segment) in segments.iter().enumerate() {
        let video_label = format!("video{}", i + 1);
        for (j, mut filter) in segment.clone().into_iter().enumerate() {
            if j > 0 {
                filter = if filter.contains("()") {
                    filter.replacen("()", &format!("({})", video_label), 1)
                } else {
                    filter.replacen("(", &&format!("({}, ", video_label), 1)
                };
            }
            writeln!(script, "{} = {}", video_label, filter).map_err(|e| e.to_string())?;
        }
        writeln!(script).map_err(|e| e.to_string())?;
    }
    writeln!(
        script,
        "{}{}",
        if output_type == OutputType::Vapoursynth {
            "video = "
        } else {
            ""
        },
        (0..segments.len())
            .map(|i| format!("video{}", i + 1))
            .collect::<Vec<String>>()
            .join(" + ")
    )
    .map_err(|e| e.to_string())?;
    if output_type == OutputType::Vapoursynth {
        writeln!(script).map_err(|e| e.to_string())?;
        writeln!(script, "video.set_output()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    use std::path::Path;

    use crate::generic::output::create_script;
    use crate::generic::output::get_default_filters;
    use crate::generic::output::AvsOptions;
    use crate::generic::output::OutputType;

    fn read_file(path: &Path) -> String {
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();
        output.replace("\r\n", "\n")
    }

    #[test]
    fn create_script_avs_basic() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_basic.avs");
        let expected = Path::new("files/avs_basic.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_audio() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_audio.avs");
        let expected = Path::new("files/avs_audio.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: false,
            ass_extract: None,
            audio: (true, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_hi10p() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_hi10p.avs");
        let expected = Path::new("files/avs_hi10p.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: true,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_cfr() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_cfr.avs");
        let expected = Path::new("files/avs_cfr.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: true,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_resize() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_resize.avs");
        let expected = Path::new("files/avs_resize.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: Some((640, 480)),
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_ass() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_ass.avs");
        let expected = Path::new("files/avs_ass.avs.expected");
        let opts = AvsOptions {
            script_type: OutputType::Avisynth,
            filters: vec![get_default_filters(OutputType::Avisynth).into()],
            ass: true,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_basic() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_basic.vpy");
        let expected = Path::new("files/vps_basic.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_audio() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_audio.vpy");
        let expected = Path::new("files/vps_audio.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: false,
            ass_extract: None,
            audio: (true, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_hi10p() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_hi10p.vpy");
        let expected = Path::new("files/vps_hi10p.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: true,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_cfr() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_cfr.vpy");
        let expected = Path::new("files/vps_cfr.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: true,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_resize() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_resize.vpy");
        let expected = Path::new("files/vps_resize.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: Some((640, 480)),
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_ass() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_ass.vpy");
        let expected = Path::new("files/vps_ass.vpy.expected");
        let opts = AvsOptions {
            script_type: OutputType::Vapoursynth,
            filters: vec![get_default_filters(OutputType::Vapoursynth).into()],
            ass: true,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            hi10p: false,
        };
        create_script(in_file, out_file, &opts).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }
}
