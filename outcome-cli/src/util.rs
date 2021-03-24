use anyhow::{Error, Result};
use std::fs;
use std::path::PathBuf;

/// Walks up the directory tree looking for the project root.
pub(crate) fn find_project_root(path: PathBuf, recursion_levels: usize) -> Result<PathBuf> {
    let mut recursion_levels = recursion_levels;
    let mut path = path;
    while recursion_levels > 0 {
        if path.is_dir() {
            for entry in fs::read_dir(&path)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    // println!("{:?}", entry.path());
                    if entry.file_name() == outcome::SCENARIOS_DIR_NAME
                        || entry.file_name() == outcome::MODULES_DIR_NAME
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
    Err(Error::msg("project root not found"))
}

/// Tries to select a scenario manifest path using a given path.
/// Basically it checks whether scenarios directory can be found in the
/// given directory path. It selects a scenario only if there is only
/// a single scenario present.
pub(crate) fn get_scenario_paths(path: PathBuf) -> Result<Vec<PathBuf>> {
    let dir_path = path.join(outcome::SCENARIOS_DIR_NAME);
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
pub(crate) fn get_snapshot_paths(path: PathBuf) -> Result<Vec<PathBuf>> {
    let dir_path = path.join(outcome::SNAPSHOTS_DIR_NAME);
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

pub(crate) fn format_elements_list(paths: &Vec<PathBuf>) -> String {
    let mut list = String::new();
    for path in paths {
        list = format!(
            "{}\n   {}",
            list,
            path.file_stem().unwrap().to_string_lossy()
        );
    }
    list
}
