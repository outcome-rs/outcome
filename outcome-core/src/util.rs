//! Contains a collection of useful utility functions.

#![allow(unused)]

extern crate strsim;

use std::collections::HashMap;

/// Walks up the directory tree looking for the project root.
pub fn find_project_root(mut path: PathBuf, recursion_levels: usize) -> Result<PathBuf> {
    let mut recursion_levels = recursion_levels;
    while recursion_levels > 0 {
        if path.is_dir() {
            for entry in fs::read_dir(&path)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    // println!("{:?}", entry.path());
                    if entry.file_name() == crate::SCENARIOS_DIR_NAME
                        || entry.file_name() == crate::MODULES_DIR_NAME
                    {
                        return Ok(path);
                    }
                }
            }
        }
        recursion_levels -= 1;
        if let Some(parent_path) = path.parent() {
            path = parent_path.to_path_buf();
        }
    }
    Err(Error::ProjectRootNotFound(
        path.to_str().unwrap().to_string(),
    ))
}

/// Tries to select a scenario manifest path using a given path.
/// Basically it checks whether scenarios directory can be found in the
/// given directory path. It selects a scenario only if there is only
/// a single scenario present.
pub fn get_scenario_paths(path: PathBuf) -> Result<Vec<PathBuf>> {
    let dir_path = path.join(crate::SCENARIOS_DIR_NAME);
    // println!("{:?}", dir_path);
    if dir_path.exists() && dir_path.is_dir() {
        let mut scenario_paths = Vec::new();
        let read_dir = fs::read_dir(dir_path).ok();
        if let Some(read_dir) = read_dir {
            for entry in read_dir {
                let entry = entry.ok();
                if let Some(entry) = entry {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(entry_ext) = entry_path.extension() {
                            if entry_ext == "toml" {
                                scenario_paths.push(entry_path);
                            }
                        }
                    }
                }
            }
        }
        if scenario_paths.len() > 0 {
            return Ok(scenario_paths);
        }
    }
    Ok(Vec::new())
}

/// Tries to select a scenario manifest path using a given path.
/// Basically it checks whether scenarios directory can be found in the
/// given directory path. It selects a scenario only if there is only
/// a single scenario present.
pub fn get_snapshot_paths(path: PathBuf) -> Result<Vec<PathBuf>> {
    let dir_path = path.join(crate::SNAPSHOTS_DIR_NAME);
    // println!("{:?}", dir_path);
    if dir_path.exists() && dir_path.is_dir() {
        let mut snapshot_paths = Vec::new();
        let read_dir = fs::read_dir(dir_path).ok();
        if let Some(read_dir) = read_dir {
            for entry in read_dir {
                let entry = entry.ok();
                if let Some(entry) = entry {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(entry_ext) = entry_path.extension() {
                            if entry_ext == ".snapshot" {
                                snapshot_paths.push(entry_path);
                            }
                        } else {
                            // warn!("snapshot without .snapshot extension");
                            snapshot_paths.push(entry_path);
                        }
                    }
                }
            }
        }
        if snapshot_paths.len() > 0 {
            return Ok(snapshot_paths);
        }
    }
    Ok(Vec::new())
}
use std::ffi::OsStr;
use std::fs::{read, read_dir, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use toml::Value;

use crate::error::Error;
use crate::Result;
use std::fs;

pub fn read_text_file(file: &str) -> std::io::Result<String> {
    let file_path = Path::new(file);
    debug!("{:?}", file_path);
    let mut fd = File::open(&file_path)?;
    let mut content = String::new();
    fd.read_to_string(&mut content)?;

    Ok(content)
}

/// Create a static deser object from given path using serde.
pub fn deser_struct_from_path<T>(file_path: PathBuf) -> Result<T>
where
    for<'de> T: serde::Deserialize<'de>,
{
    let bytes = read(file_path.clone())?;
    let d: T = match file_path.extension().unwrap().to_str().unwrap() {
        "toml" => toml::from_slice(&bytes)?,
        #[cfg(feature = "yaml")]
        "yaml" | "yml" => serde_yaml::from_slice(&bytes)?,
        _ => unimplemented!(),
    };
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

/// Get paths to files with any of the given extensions in the provided
/// directory.
pub fn find_files_with_extension(
    dir: PathBuf,
    extensions: Vec<&str>,
    recursive: bool,
    exclude: Option<Vec<String>>,
) -> Vec<PathBuf> {
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
            if path.is_dir() && recursive {
                paths.extend(find_files_with_extension(
                    path,
                    extensions.clone(),
                    recursive,
                    exclude.clone(),
                ));
            } else if path.is_file() {
                let ext = path
                    .extension()
                    .unwrap_or(OsStr::new(""))
                    .to_str()
                    .unwrap_or("");
                for extension in &extensions {
                    if &ext == extension {
                        // TODO excludes
                        //if let Some(excludes) = exclude {
                        //for exclude in excludes {
                        //if path.file_name().unwrap_or(OsStr::new("")) == exclude {
                        ////
                        //}
                        //}
                        //}
                        paths.push(path.clone());
                        break;
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
/// Reads a file at the given path to a String.
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

/// Coerces serde_yaml value to string.
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
/// Truncates string to specified size (ignoring last bytes if they form a partial `char`).
#[inline]
pub(crate) fn truncate_str(slice: &str, size: u8) -> &str {
    if slice.is_char_boundary(size.into()) {
        unsafe { slice.get_unchecked(..size.into()) }
    } else if (size as usize) < slice.len() {
        let mut index = size.saturating_sub(1) as usize;
        while !slice.is_char_boundary(index) {
            index = index.saturating_sub(1);
        }
        unsafe { slice.get_unchecked(..index) }
    } else {
        slice
    }
}
