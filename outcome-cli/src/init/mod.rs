//! Initialize files and projects based on templates.

#![allow(unused_imports)]

pub mod module;
pub mod proof;
pub mod scenario;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Error, Result};

// Initiate new content structure template based on input args
pub fn init_at_path(type_str: &str, path_str: &str, template_str: &str) -> Result<()> {
    println!(
        "Initiating new {type_} at: {path} (template: {template}) ",
        type_ = type_str,
        path = path_str,
        template = template_str
    );

    // get the file stem as module name
    let path = Path::new(path_str);
    let name = path.file_stem().unwrap().to_str().unwrap();

    // test if directory doesn't already exist at path
    if path.exists() {
        return Err(Error::msg(format!(
            "Can't initialize {type_}, directory already exists ({path}). Try another path.",
            type_ = type_str,
            path = path_str
        )));
    }

    // get the template files
    let template_files = match collect_template_files(type_str, name, template_str) {
        Some(tf) => tf,
        None => {
            return Err(Error::msg(format!(
                "Failed getting {} template files for template \"{}\"",
                type_str, template_str
            )))
        }
    };

    // create the new directory for the module
    fs::create_dir_all(path_str).unwrap();

    // create the template files
    create_template_files(path_str, template_files);

    Ok(())
}

// Create actual files from the template file content
fn create_template_files(path_str: &str, files: HashMap<String, String>) {
    for (name, content) in files {
        let module_path = Path::new(path_str);
        let file_full_path = module_path.join(name);
        //let mut file = File::create(file_full_path).unwrap();
        // file.write_fmt(content).unwrap();
        use std::fs;
        fs::create_dir_all(file_full_path.parent().unwrap());
        fs::write(file_full_path, content)
            .expect(format!("Failed to create a template \"{}\"", path_str).as_str());
    }
}

// Collect the necessary template files based on init type, name and template_str
fn collect_template_files(
    type_str: &str,
    name: &str,
    template_str: &str,
) -> Option<HashMap<String, String>> {
    match type_str {
        "scenario" => scenario::collect_template_files(name, template_str),
        "module" => module::collect_template_files(name, template_str),
        "proof" => proof::collect_template_files(name, template_str),
        _ => None,
    }
}
