use std::fs::File;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

pub struct AvsOptions {
    pub remove_grain: Option<u8>
}

pub fn create_avs_script(in_file: PathBuf, out_file: PathBuf, opts: AvsOptions) -> Result<(), Box<Error>> {
    let mut script = try!(File::create(out_file));
    writeln!(&mut script, "FFVideoSource(\"{}\")", in_file.to_str().unwrap()).unwrap();
    if let Some(remove_grain) = opts.remove_grain {
        writeln!(&mut script, "RemoveGrain({})", remove_grain).unwrap();
    }

    Ok(())
}
