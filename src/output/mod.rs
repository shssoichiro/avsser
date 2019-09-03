use super::input::InputTypes;
use crate::input::determine_input_type;
use crate::parsers::mkvinfo::get_file_uuid;
use crate::parsers::mkvinfo::get_fonts_list;
use crate::parsers::mkvinfo::get_ordered_chapters_list;
use crate::parsers::mkvinfo::BreakPoint;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

mod avisynth;
mod vapoursynth;

pub use avisynth::*;
pub use vapoursynth::*;

pub trait ScriptFormat {
    fn create_script(&mut self, in_file: &Path, out_file: &Path) -> Result<(), String> {
        let breakpoints = get_ordered_chapters_list(in_file, self.get_opts().to_cfr)?;
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
                        return Err(
                            "Could not find file linked through ordered chapters.".to_owned()
                        );
                    }
                }
            }

            let mut current_filters = Vec::new();
            if self.get_opts().to_cfr && !preloads.contains_key(&current_filename) {
                preloads.insert(
                    current_filename.clone(),
                    self.build_video_filter_string(&current_filename, true),
                );
            }
            current_filters.push(self.build_video_filter_string(&current_filename, false));
            if self.get_opts().to_cfr {
                // This needs to happen before the `AudioDub`
                // Also, `vfrtocfr` requires the full path to the timecodes file
                current_filters
                    .push(self.build_vfr_string(&current_filename.with_extension("timecodes.txt")));
            }
            let audio = self.get_opts().audio.clone();
            match audio {
                (false, None) => (),
                (true, None) => {
                    current_filters.push(
                        self.build_audio_dub_string(&current_filename.canonicalize().unwrap()),
                    );
                }
                (_, Some(ref x)) => {
                    current_filters.push(self.build_audio_dub_string(
                        &current_filename.with_extension(x).canonicalize().unwrap(),
                    ));
                }
            }
            if !self.get_opts().filters.is_empty() {
                current_filters.extend_from_slice(&self.get_opts().filters);
            }
            if let Some(sub_track) = self.get_opts().ass_extract {
                if current_filename.with_extension("ass").exists() {
                    println!(
                        "Cowardly refusing to overwrite existing subtitles: {}",
                        current_filename.with_extension("ass").to_string_lossy()
                    );
                } else {
                    extract_subtitles(current_filename.as_ref(), sub_track)?;
                }
            }
            if self.get_opts().ass {
                let ass_file = current_filename
                    .with_extension("ass")
                    .canonicalize()
                    .unwrap();
                current_filters.push(self.build_subtitle_string(&ass_file));
            }
            if let Some((width, height)) = self.get_opts().resize {
                current_filters.push(self.build_resize_string(width, height));
            }
            if breakpoints.is_some() {
                current_filters.push(self.build_trim_string(current_breakpoint.unwrap()));
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

        self.write_script_header(&mut script)?;

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

        self.write_segments(&segments, &mut script)
    }

    fn get_opts(&self) -> &AvsOptions;

    fn get_script_extension(&self) -> &'static str;

    fn build_video_filter_string(&self, current_filename: &Path, is_preload: bool) -> String;

    fn build_vfr_string(&self, timecodes_path: &Path) -> String;

    fn build_audio_dub_string(&mut self, audio_filename: &Path) -> String;

    fn build_subtitle_string(&self, subtitle_filename: &Path) -> String;

    fn build_resize_string(&self, width: u32, height: u32) -> String;

    fn build_trim_string(&self, breakpoint: BreakPoint) -> String;

    fn write_script_header(&self, _script: &mut File) -> Result<(), String> {
        // Default to writing no header
        Ok(())
    }

    fn write_segments(&self, segments: &[Vec<String>], script: &mut File) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct AvsOptions {
    pub filters: Vec<String>,
    pub ass: bool,
    pub ass_extract: Option<u8>,
    pub audio: (bool, Option<String>),
    pub resize: Option<(u32, u32)>,
    pub to_cfr: bool,
    pub downsample: bool,
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
