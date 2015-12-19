use std::fs;
use std::path::PathBuf;

pub enum InputTypes {
    Matroska,
    Mpeg4,
    Avi,
    Other
}

pub fn get_list_of_files(path: PathBuf, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path]);
    }
    if !path.is_dir() {
        return Err("Cannot handle file, perhaps it's a symlink or you don't have proper permissions?".to_owned());
    }
    let mut files: Vec<PathBuf> = vec![];
    get_recursive_files(path, &mut files, recursive);
    Ok(files)
}

fn get_recursive_files(path: PathBuf, mut files: &mut Vec<PathBuf>, recursive: bool) {
    let paths = fs::read_dir(path).unwrap();
    for path in paths {
        let next = path.unwrap().path();
        if next.is_file() {
            files.push(next.clone());
        }
        if recursive && next.is_dir() {
            get_recursive_files(next, &mut files, recursive);
        }
    }
}

pub fn determine_input_type(path: PathBuf) -> Option<InputTypes> {
    // This is simplistic and assumes that the extension is a source of truth
    // TODO: Make this look at the container headers instead
    let extension = path.extension().unwrap().to_str().unwrap().to_lowercase();
    match extension.as_ref() {
        "mkv" => Some(InputTypes::Matroska),
        "mp4" => Some(InputTypes::Mpeg4),
        "avi" => Some(InputTypes::Avi),
        "mpeg" => Some(InputTypes::Other),
        "mpg" => Some(InputTypes::Other),
        "wmv" => Some(InputTypes::Other),
        "mov" => Some(InputTypes::Other),
        "flv" => Some(InputTypes::Other),
        "webm" => Some(InputTypes::Other),
        _ => None
    }
}
