use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum InputTypes {
    Matroska,
    Mpeg4,
    Avi,
    DgIndex,
    DgAvc,
    Other,
}

pub fn get_list_of_files(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_owned()]);
    }
    if !path.is_dir() {
        return Err(
            "Cannot handle file, perhaps it's a symlink or you don't have proper \
             permissions?"
                .to_owned(),
        );
    }
    let mut files: Vec<PathBuf> = vec![];
    get_recursive_files(path, &mut files, recursive);
    Ok(files)
}

fn get_recursive_files(path: &Path, mut files: &mut Vec<PathBuf>, recursive: bool) {
    let paths = fs::read_dir(path).unwrap();
    for path in paths {
        let next = path.unwrap().path();
        if next.is_file() {
            files.push(next.clone());
        }
        if recursive && next.is_dir() {
            get_recursive_files(next.as_ref(), &mut files, recursive);
        }
    }
}

pub fn determine_input_type(path: &Path) -> Option<InputTypes> {
    // This is simplistic and assumes that the extension is a source of truth
    // TODO: Make this look at the container headers instead
    let extension = path.extension().unwrap().to_str().unwrap().to_lowercase();
    match extension.as_ref() {
        "mkv" => Some(InputTypes::Matroska),
        "mp4" => Some(InputTypes::Mpeg4),
        "avi" => Some(InputTypes::Avi),
        "d2v" => Some(InputTypes::DgIndex),
        "dga" => Some(InputTypes::DgAvc),
        "mpeg" | "mpg" | "wmv" | "mov" | "flv" | "webm" | "ivf" => Some(InputTypes::Other),
        _ => None,
    }
}
