use std::path::Path;
use std::collections::HashMap;
use std::process::Command;
use regex::Regex;

pub fn get_fonts_list(path: &Path) -> Result<HashMap<usize, String>, String> {
    let output = match Command::new("mkvmerge")
        .args(&["-i",
            format!("{}", path.to_str().unwrap()).as_ref()])
        .output() {
            Ok(x) => x,
            Err(x) => return Err(format!("{}", x))
        };

    let mut attachments: HashMap<usize, String> = HashMap::new();
    let pattern = Regex::new(r"Attachment ID (\d+): .* file name '(.+)'").unwrap();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        if line.starts_with("Attachment") && (line.contains(".ttf") || line.contains(".otf")) {
            let captures = pattern.captures(line).unwrap();
            attachments.insert(
                captures.at(1).unwrap().parse::<usize>().unwrap(),
                captures.at(2).unwrap().to_owned()
            );
        }
    }

    Ok(attachments)
}
