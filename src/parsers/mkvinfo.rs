use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::process::Command;
use regex::Regex;
use rustc_serialize::hex::FromHex;

pub fn get_fonts_list(path: &Path) -> Result<HashMap<usize, String>, String> {
    let output = match Command::new("mkvmerge")
                           .args(&["-i", path.to_str().unwrap().as_ref()])
                           .output() {
        Ok(x) => x,
        Err(x) => return Err(format!("{}", x)),
    };

    let mut attachments: HashMap<usize, String> = HashMap::new();
    let pattern = Regex::new(r"Attachment ID (\d+): .* file name '(.+)'").unwrap();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        if line.starts_with("Attachment") && (line.contains(".ttf") || line.contains(".otf")) {
            let captures = pattern.captures(line).unwrap();
            attachments.insert(captures.at(1).unwrap().parse::<usize>().unwrap(),
                               captures.at(2).unwrap().to_owned());
        }
    }

    Ok(attachments)
}

pub fn get_file_uuid(path: &Path) -> Result<[u8; 16], String> {
    let uuid_regex = Regex::new(
        r"Segment UID: 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2})"
    ).unwrap();

    let output = match Command::new("mkvinfo")
                           .args(&[path.to_str().unwrap()])
                           .output() {
        Ok(x) => x,
        Err(x) => return Err(x.description().to_owned()),
    };

    let output = String::from_utf8(output.stdout).unwrap();
    for line in output.lines() {
        if let Some(captures) = uuid_regex.captures(line) {
            return Ok([captures[1].from_hex().unwrap()[0],
                       captures[2].from_hex().unwrap()[0],
                       captures[3].from_hex().unwrap()[0],
                       captures[4].from_hex().unwrap()[0],
                       captures[5].from_hex().unwrap()[0],
                       captures[6].from_hex().unwrap()[0],
                       captures[7].from_hex().unwrap()[0],
                       captures[8].from_hex().unwrap()[0],
                       captures[9].from_hex().unwrap()[0],
                       captures[10].from_hex().unwrap()[0],
                       captures[11].from_hex().unwrap()[0],
                       captures[12].from_hex().unwrap()[0],
                       captures[13].from_hex().unwrap()[0],
                       captures[14].from_hex().unwrap()[0],
                       captures[15].from_hex().unwrap()[0],
                       captures[16].from_hex().unwrap()[0]]);
        }
    }

    Err(format!("No uuid found in {}, is this a valid Matroska file?",
                path.to_str().unwrap()))
}

#[derive(Clone,Debug)]
pub struct BreakPoint {
    pub start_frame: u64,
    pub end_frame: u64,
    pub foreign_uuid: Option<[u8; 16]>,
}

pub fn get_ordered_chapters_list(path: &Path) -> Result<Option<Vec<BreakPoint>>, String> {
    let output = match Command::new("mkvinfo")
                           .args(&[path.to_str().unwrap()])
                           .output() {
        Ok(x) => x,
        Err(x) => return Err(x.description().to_owned()),
    };

    let output = String::from_utf8(output.stdout).unwrap();
    let mut chapters: Vec<BreakPoint> = Vec::new();
    let mut video_fps: Option<f64> = None;
    let fps_pattern = Regex::new(r"Default duration:.+\((\d+\.\d+) frames/fields per second")
                          .unwrap();
    let time_start_regex = Regex::new(r"ChapterTimeStart: (\d{2}):(\d{2}):(\d{2}).(\d{9})")
                               .unwrap();
    let time_end_regex = Regex::new(r"ChapterTimeEnd: (\d{2}):(\d{2}):(\d{2}).(\d{9})").unwrap();
    // I'm not that good at regex
    let foreign_uuid_regex = Regex::new(
        r"ChapterSegmentUID: length 16, data: 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2}) 0x([0-9a-f]{2})"
    ).unwrap();
    let mut current_section: Option<String> = None;
    let mut current_chapter: Option<BreakPoint> = None;
    let mut ordered_chapters = false;
    for line in output.lines() {
        // Find video_fps
        if video_fps.is_none() {
            if current_section == Some("video".to_owned()) {
                if let Some(captures) = fps_pattern.captures(line) {
                    video_fps = Some(captures.at(1).unwrap().parse::<f64>().unwrap());
                }
            } else if line == "|  + Track type: video" {
                current_section = Some("video".to_owned());
            }
            continue;
        }
        if current_section == Some("chapters".to_owned()) {
            if line == "|  + EditionFlagOrdered: 1" {
                ordered_chapters = true;
                continue;
            }
            if line == "|  + ChapterAtom" {
                if current_chapter.is_some() {
                    chapters.push(current_chapter.unwrap().clone());
                }
                current_chapter = Some(BreakPoint {
                    start_frame: 0,
                    end_frame: 0,
                    foreign_uuid: None,
                });
                continue;
            }
            if current_chapter.is_some() {
                if let Some(captures) = time_start_regex.captures(line) {
                    current_chapter.as_mut().unwrap().start_frame =
                        timestamp_to_frame_number(captures.at(1).unwrap().parse::<u64>().unwrap(),
                                                  captures.at(2).unwrap().parse::<u64>().unwrap(),
                                                  captures.at(3).unwrap().parse::<f64>().unwrap() +
                                                  captures.at(4).unwrap().parse::<f64>().unwrap() /
                                                  1000000000f64,
                                                  video_fps.unwrap());
                    continue;
                }
                if let Some(captures) = time_end_regex.captures(line) {
                    current_chapter.as_mut().unwrap().end_frame =
                        timestamp_to_frame_number(captures.at(1).unwrap().parse::<u64>().unwrap(),
                                                  captures.at(2).unwrap().parse::<u64>().unwrap(),
                                                  captures.at(3).unwrap().parse::<f64>().unwrap() +
                                                  captures.at(4).unwrap().parse::<f64>().unwrap() /
                                                  1000000000f64,
                                                  video_fps.unwrap());
                    continue;
                }
                if let Some(captures) = foreign_uuid_regex.captures(line) {
                    current_chapter.as_mut().unwrap().foreign_uuid =
                        Some([captures[1].from_hex().unwrap()[0],
                              captures[2].from_hex().unwrap()[0],
                              captures[3].from_hex().unwrap()[0],
                              captures[4].from_hex().unwrap()[0],
                              captures[5].from_hex().unwrap()[0],
                              captures[6].from_hex().unwrap()[0],
                              captures[7].from_hex().unwrap()[0],
                              captures[8].from_hex().unwrap()[0],
                              captures[9].from_hex().unwrap()[0],
                              captures[10].from_hex().unwrap()[0],
                              captures[11].from_hex().unwrap()[0],
                              captures[12].from_hex().unwrap()[0],
                              captures[13].from_hex().unwrap()[0],
                              captures[14].from_hex().unwrap()[0],
                              captures[15].from_hex().unwrap()[0],
                              captures[16].from_hex().unwrap()[0]]);
                    continue;
                }
            }
            if line.starts_with("|+ EbmlVoid") {
                if current_chapter.is_some() {
                    chapters.push(current_chapter.unwrap().clone());
                }
                break;
            }
        } else if line == "|+ Chapters" {
            current_section = Some("chapters".to_owned());
            continue;
        }
    }

    if !ordered_chapters {
        return Ok(None);
    }

    // Merge chapters
    let mut breakpoints: Vec<BreakPoint> = Vec::new();
    let mut iter = chapters.iter().peekable();
    let mut merging = BreakPoint {
        start_frame: 0,
        end_frame: 0,
        foreign_uuid: None,
    };
    while let Some(chapter) = iter.next() {
        if chapter.foreign_uuid.is_some() {
            breakpoints.push(chapter.clone());
            continue;
        }
        if merging.end_frame == 0 {
            merging.start_frame = chapter.start_frame;
        }
        merging.end_frame = chapter.end_frame;
        if let Some(next_chapter) = iter.peek() {
            if next_chapter.foreign_uuid.is_some() && merging.end_frame > 0 {
                breakpoints.push(merging.clone());
                merging = BreakPoint {
                    start_frame: 0,
                    end_frame: 0,
                    foreign_uuid: None,
                };
            }
        } else {
            if merging.end_frame > 0 {
                breakpoints.push(merging.clone());
            }
            break;
        }
    }

    Ok(Some(breakpoints))
}

fn timestamp_to_frame_number(hours: u64, minutes: u64, seconds: f64, fps: f64) -> u64 {
    ((seconds + 60f64 * minutes as f64 + 3600f64 * hours as f64) * fps).floor() as u64
}
