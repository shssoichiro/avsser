use super::*;
use std::fs::File;
use std::path::Path;

pub struct VapoursynthWriter {
    opts: AvsOptions,
    audio_filename: Option<PathBuf>,
}

impl ScriptFormat for VapoursynthWriter {
    fn build_video_filter_string(&self, current_filename: &Path, is_preload: bool) -> String {
        let video_filter = self.get_video_filter_full_name(&current_filename);
        let timecodes_path = current_filename.with_extension("timecodes.txt");
        if self.opts.to_cfr && !timecodes_path.exists() {
            File::create(&timecodes_path).ok();
        }
        let mut filter_opts = String::new();
        if self.opts.to_cfr && is_preload {
            filter_opts.push_str(&format!(
                ", timecodes=\"{}\"",
                escape_python_string(timecodes_path.canonicalize().unwrap().to_str().unwrap()),
            ));
        }

        let mut source_filter = format!(
            "{}({}{})",
            video_filter,
            format!(
                "source='{}'",
                escape_python_string(current_filename.canonicalize().unwrap().to_str().unwrap())
            ),
            filter_opts
        );
        if self.opts.downsample {
            source_filter.push_str("\nvideo1 = core.resize.Spline36(video1, format = vs.YUV420P8)");
        }
        source_filter
    }

    fn build_vfr_string(&self, timecodes_path: &Path) -> String {
        format!(
            "vfrtocfr.VFRToCFR(\"{}\", 120000, 1001)",
            escape_python_string(timecodes_path.canonicalize().unwrap().to_str().unwrap())
        )
    }

    #[inline(always)]
    fn get_opts(&self) -> &AvsOptions {
        &self.opts
    }

    #[inline(always)]
    fn get_script_extension(&self) -> &'static str {
        "vpy"
    }

    fn build_audio_dub_string(&mut self, audio_filename: &Path) -> String {
        self.audio_filename = Some(audio_filename.to_path_buf());
        format!(
            "core.damb.Read(\'{}\')",
            escape_python_string(audio_filename.to_str().unwrap())
        )
    }

    fn build_subtitle_string(&self, subtitle_filename: &Path) -> String {
        format!(
            "core.sub.TextFile(\'{}\')",
            escape_python_string(subtitle_filename.to_str().unwrap())
        )
    }

    fn build_resize_string(&self, width: u32, height: u32) -> String {
        format!("core.resize.Spline36({}, {})", width, height)
    }

    fn build_trim_string(&self, breakpoint: BreakPoint) -> String {
        format!(
            "core.std.Trim({}, {})",
            breakpoint.start_frame, breakpoint.end_frame
        )
    }

    fn write_script_header(&self, script: &mut File) -> Result<(), String> {
        writeln!(script, "import vapoursynth as vs").map_err(|e| e.to_string())?;
        writeln!(script, "core = vs.get_core()").map_err(|e| e.to_string())?;
        writeln!(script).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn write_segments(&self, segments: &[Vec<String>], script: &mut File) -> Result<(), String> {
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
            "video = {}",
            (0..segments.len())
                .map(|i| format!("video{}", i + 1))
                .collect::<Vec<String>>()
                .join(" + ")
        )
        .map_err(|e| e.to_string())?;
        writeln!(script).map_err(|e| e.to_string())?;
        if let Some(ref audio_filename) = self.audio_filename {
            writeln!(
                script,
                "core.damb.Write(video, \'{}\')",
                escape_python_string(audio_filename.with_extension("flac").to_str().unwrap())
            )
            .map_err(|e| e.to_string())?;
        }
        writeln!(script, "video.set_output()").map_err(|e| e.to_string())?;

        Ok(())
    }
}

fn escape_python_string(input: &str) -> String {
    input.replace(r"\", r"\\").replace(r"'", r"\'")
}

impl VapoursynthWriter {
    pub fn new(mut opts: AvsOptions, apply_default_filters: bool) -> Self {
        let default_filters: &[String] = &["core.rgvs.RemoveGrain(1)".to_string()];
        if apply_default_filters {
            opts.filters.extend_from_slice(default_filters);
        }
        VapoursynthWriter {
            opts,
            audio_filename: Default::default(),
        }
    }

    fn determine_video_source_filter(&self, path: &Path) -> &'static str {
        match determine_input_type(path) {
            Some(InputTypes::DgIndex) => "core.d2v.Source",
            Some(InputTypes::DgAvc) => unimplemented!(),
            Some(_) => "core.ffms2.Source",
            None => panic!("Invalid input type"),
        }
    }

    fn get_video_filter_full_name(&self, current_filename: &Path) -> &'static str {
        self.determine_video_source_filter(&current_filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    use std::path::Path;

    fn read_file(path: &Path) -> String {
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();
        output.replace("\r\n", "\n")
    }

    #[test]
    fn create_script_vps_basic() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_basic.vpy");
        let expected = Path::new("files/vps_basic.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_audio() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_audio.vpy");
        let expected = Path::new("files/vps_audio.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (true, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_downsample() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_downsample.vpy");
        let expected = Path::new("files/vps_downsample.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: true,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_cfr() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_cfr.vpy");
        let expected = Path::new("files/vps_cfr.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: true,
            downsample: false,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_resize() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_resize.vpy");
        let expected = Path::new("files/vps_resize.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: Some((640, 480)),
            to_cfr: false,
            downsample: false,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_vps_ass() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/vps_ass.vpy");
        let expected = Path::new("files/vps_ass.vpy.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: true,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = VapoursynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }
}
