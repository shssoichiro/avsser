#![warn(clippy::all)]

pub mod input;
pub mod output;

pub mod parsers {
    pub mod ffprobe;
    pub mod mkvinfo;
}
