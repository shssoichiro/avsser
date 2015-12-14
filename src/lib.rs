#![feature(plugin)]
#![plugin(clippy)]

pub mod generic {
    pub mod input;
    pub mod output;
}

mod parsers {
    mod ffprobe;
}
