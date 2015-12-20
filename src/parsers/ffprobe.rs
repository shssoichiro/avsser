use std::path::Path;
use std::collections::HashMap;
use std::process::Command;

pub fn get_streams_list(path: &Path) -> Result<Vec<HashMap<String, String>>, String> {
    let output = match Command::new("ffprobe")
        .args(&["-show_streams",
            format!("{}", path.to_str().unwrap()).as_ref()])
        .output() {
            Ok(x) => x,
            Err(x) => return Err(format!("{}", x))
        };

    let mut streams: Vec<HashMap<String, String>> = Vec::new();
    let mut current: HashMap<String, String> = HashMap::new();
    let mut relevant = false;
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        if line == "[STREAM]" {
            relevant = true;
            current = HashMap::new();
        } else if !relevant {
            continue;
        } else if line == "[/STREAM]" {
            streams.push(current.clone());
        } else {
            let key_pair = line.splitn(2, '=').collect::<Vec<&str>>();
            current.insert(key_pair[0].to_owned(), key_pair[1].to_owned());
        }
    }

    Ok(streams)
}
