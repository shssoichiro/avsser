#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate uuid;

pub mod generic {
    pub mod input;
    pub mod output;
}

pub mod parsers {
    pub mod ffprobe;
    pub mod mkvinfo;
}
