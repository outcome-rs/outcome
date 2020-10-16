//! Contains a collection of useful utility functions.

#![allow(unused)]

extern crate strsim;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{read, read_dir, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use toml::Value;

use crate::error::Error;
use crate::MedString;
use crate::Result;

pub fn read_text_file(file: &str) -> std::io::Result<String> {
    let file_path = Path::new(file);
    debug!("{:?}", file_path);
    let mut fd = File::open(&file_path)?;
    let mut content = String::new();
    fd.read_to_string(&mut content)?;

    Ok(content)
}

/// Create a static deser object from given path using
/// serde.
pub fn static_deser_obj_from_path<T>(file_path: PathBuf) -> Result<T>
where
    for<'de> T: serde::Deserialize<'de>,
{
    let file_data = read(file_path)?;
    let d: T = toml::from_slice(&file_data)?;
    Ok(d)
}

/// Get top level directories at the given path.
pub fn get_top_dirs_at(dir: PathBuf) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if dir.is_dir() {
        let dir_entry = match read_dir(&dir) {
            Ok(d) => d,
            _ => {
                error!("couldn't read directory at path: {}", dir.to_string_lossy());
                return Vec::new();
            }
        };
        for entry in dir_entry {
            let path = match entry {
                Ok(p) => p.path(),
                _ => continue,
            };
            if path.is_dir() {
                paths.push(path);
            }
        }
    };
    paths
}

/// Get paths to files with `yaml` or `yml` extension in a
/// directory (recursive).
pub fn get_yaml_files_at_dir(dir: PathBuf, exclude: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if dir.is_dir() {
        let dir_entry = match read_dir(&dir) {
            Ok(d) => d,
            _ => {
                error!("couldn't read directory at path: {}", dir.to_string_lossy());
                return Vec::new();
            }
        };
        for entry in dir_entry {
            let path = match entry {
                Ok(p) => p.path(),
                _ => continue,
            };
            if path.is_dir() {
                paths.extend(get_yaml_files_at_dir(path, exclude));
            } else if path.is_file() {
                let extension = path.extension().unwrap_or(OsStr::new(""));
                if extension == "yaml" || extension == "yml" {
                    if path.file_name().unwrap_or(OsStr::new("")) != exclude {
                        paths.push(path.clone());
                    }
                }
            }
        }
    };
    paths
}

// /// Deserialize an object at the given path.
// pub fn deser_obj_from_path<T>(file_path: PathBuf) -> Result<T>
// where
//     for<'de> T: serde::Deserialize<'de>,
// {
//     let file_data = read(file_path)?;
//     let d: T = serde_yaml::from_slice(&file_data)?;
//     Ok(d)
// }
/// Read a file at the given path to a String.
pub fn read_file(path: &str) -> Result<String> {
    // Create a path to the desired file
    let path = Path::new(path);
    let display = path.display();
    // info!("Reading file: {}", display);

    // Open the path in read-only mode, returns
    // `io::Result<File>`
    let mut file = File::open(&path)?;

    // Read the file contents into a string, returns
    // `io::Result<usize>`
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

/// Coerce serde_yaml value to string.
pub fn coerce_toml_val_to_string(val: &Value) -> String {
    match val {
        Value::String(v) => v.to_string(),
        Value::Float(v) => format!("{}", v),
        Value::Integer(v) => format!("{}", v),
        Value::Boolean(v) => format!("{}", v),
        Value::Array(v) => format!("{:?}", v),
        Value::Table(v) => format!("{:?}", v),
        _ => unimplemented!(),
    }
}

pub fn str_from_map_value(key: &str, serde_value: &HashMap<String, Value>) -> Result<String> {
    match serde_value.get(key) {
        Some(val) => match val.as_str() {
            Some(s) => Ok(s.to_owned()),
            None => Err(Error::Other(format!(
                "value at \"{}\" must be a string",
                key
            ))),
        },
        None => Err(Error::Other(format!("map doesn't contain \"{}\"", key))),
    }
}

/// Get a similar command based on string similarity.
pub fn get_similar(original_cmd: &str, cmd_list: &[&str]) -> Option<String> {
    use self::strsim::{jaro, normalized_damerau_levenshtein};
    //        let command_list = CMD_LIST;
    let mut highest_sim = 0f64;
    let mut best_cmd_string = cmd_list[0];
    for cmd in cmd_list {
        let mut j = normalized_damerau_levenshtein(cmd, original_cmd);
        if j > highest_sim {
            highest_sim = j;
            best_cmd_string = &cmd;
        }
    }
    if highest_sim > 0.4f64 {
        //            println!("{}", highest_sim);
        Some(best_cmd_string.to_owned())
    } else {
        None
    }
}
