use super::*;
use std::path::Path;

pub struct AvisynthWriter {
    opts: AvsOptions,
}

impl ScriptFormat for AvisynthWriter {
    fn build_video_filter_string(&self, current_filename: &Path, is_preload: bool) -> String {
        let video_filter = self.get_video_filter_full_name(&current_filename);
        let timecodes_path = current_filename.with_extension("timecodes.txt");
        if self.opts.to_cfr && !timecodes_path.exists() {
            File::create(&timecodes_path).ok();
        }
        let mut filter_opts = String::new();
        if self.opts.downsample {
            filter_opts.push_str(", format = \"YUV420P8\"");
        }
        if self.opts.to_cfr && is_preload {
            filter_opts.push_str(&format!(
                ", timecodes=\"{}\"",
                timecodes_path
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            ));
        }

        format!(
            "{}({}{})",
            video_filter,
            format!(
                "\"{}\"",
                current_filename.canonicalize().unwrap().to_str().unwrap()
            ),
            filter_opts
        )
    }

    #[inline(always)]
    fn get_opts(&self) -> &AvsOptions {
        &self.opts
    }

    #[inline(always)]
    fn get_script_extension(&self) -> &'static str {
        "avs"
    }

    fn build_downsample_string(&self) -> Option<String> {
        None
    }

    fn build_vfr_string(&self, timecodes_path: &Path) -> String {
        format!(
            "vfrtocfr(timecodes=\"{}\", fpsnum=120000, fpsden=1001)",
            timecodes_path.canonicalize().unwrap().to_str().unwrap()
        )
    }

    fn build_audio_dub_string(&mut self, audio_filename: &Path) -> String {
        format!(
            "AudioDub(FFAudioSource(\"{}\"))",
            audio_filename.to_str().unwrap()
        )
    }

    fn build_subtitle_string(&self, subtitle_filename: &Path) -> String {
        format!("TextSub(\"{}\")", subtitle_filename.to_str().unwrap())
    }

    fn build_resize_string(&self, width: u32, height: u32) -> String {
        format!("Spline64Resize({}, {})", width, height)
    }

    fn build_trim_string(&self, breakpoint: BreakPoint) -> String {
        format!("Trim({},{})", breakpoint.start_frame, breakpoint.end_frame)
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
            "{}",
            (0..segments.len())
                .map(|i| format!("video{}", i + 1))
                .collect::<Vec<String>>()
                .join(" + ")
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}

impl AvisynthWriter {
    pub fn new(mut opts: AvsOptions, apply_default_filters: bool) -> Self {
        let default_filters: &[String] = &["RemoveGrain(1)".to_string()];
        if apply_default_filters {
            opts.filters.extend_from_slice(default_filters);
        }
        AvisynthWriter { opts }
    }

    fn determine_video_source_filter(&self, path: &Path) -> &'static str {
        match determine_input_type(path) {
            Some(InputTypes::DgIndex) => "DGDecode_MPEG2Source",
            Some(InputTypes::DgAvc) => "AVCSource",
            Some(_) => "FFVideoSource",
            None => panic!("Invalid input type"),
        }
    }

    fn get_video_filter_full_name(&self, current_filename: &Path) -> &'static str {
        if self.opts.downsample {
            "LWLibAvVideoSource"
        } else {
            self.determine_video_source_filter(&current_filename)
        }
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
    fn create_script_avs_basic() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_basic.avs");
        let expected = Path::new("files/avs_basic.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_audio() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_audio.avs");
        let expected = Path::new("files/avs_audio.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (true, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_downsample() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_downsample.avs");
        let expected = Path::new("files/avs_downsample.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: true,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_cfr() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_cfr.avs");
        let expected = Path::new("files/avs_cfr.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: true,
            downsample: false,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_resize() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_resize.avs");
        let expected = Path::new("files/avs_resize.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: false,
            ass_extract: None,
            audio: (false, None),
            resize: Some((640, 480)),
            to_cfr: false,
            downsample: false,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }

    #[test]
    fn create_script_avs_ass() {
        let in_file = Path::new("files/example.mkv");
        let out_file = Path::new("files/avs_ass.avs");
        let expected = Path::new("files/avs_ass.avs.expected");
        let opts = AvsOptions {
            filters: vec![],
            ass: true,
            ass_extract: None,
            audio: (false, None),
            resize: None,
            to_cfr: false,
            downsample: false,
        };
        let mut writer = AvisynthWriter::new(opts, true);
        writer.create_script(in_file, out_file).unwrap();
        assert_eq!(&read_file(out_file), &read_file(expected));
    }
}
