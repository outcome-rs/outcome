use anyhow::{Error, Result};
use std::fs;
use std::path::PathBuf;

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
