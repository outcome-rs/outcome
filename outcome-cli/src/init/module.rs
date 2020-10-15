use std::collections::HashMap;

pub fn collect_template_files(name: &str, template_str: &str) -> Option<HashMap<String, String>> {
    match template_str {
        "commented" => Some(template_commented(name, "")),
        _ => None,
    }
}

// TODO develop the templates

// commented template
pub fn template_commented(module_name: &str, precede_path: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        format!("{}/{}", precede_path, "module.yaml"),
        format!(
            r##"
# name of the module, has to be snake_case without spaces (required)
name: {name}
# version of the module (required)
version: 0.1.0
# version of the outcome engine (required)
engine: 0.2.0
# user-centric title (optional)
title: {title}
# short description (optional)
desc: This is a simple module.
# longer description (optional)
desc_long: |
    This is a longer description of the module.
    It's a simple module created with endgame's `init` subcommand.
# author of the scenario (optional)
author: You
# website of the author (optional)
website: theoutcomeproject.com
# module dependencies (optional)
# dependencies:
# - other_mod: 0.1.0
"##,
            name = module_name.replace(" ", "_"),
            title = module_name.replace("_", " "),
        ),
    );

    map
}
