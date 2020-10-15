use super::module;
use std::collections::HashMap;

pub fn collect_template_files(name: &str, template_str: &str) -> Option<HashMap<String, String>> {
    match template_str {
        "commented" => Some(template_commented(name)),
        "tutorial" => Some(template_tutorial(name)),
        _ => None,
    }
}

// TODO develop the templates

// commented template
fn template_commented(name: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "scenario.yaml".to_string(),
        format!(
            r##"
# name should be snake_case
name: {name}
# version of the scenario (required)
version: 0.1.0
# version of the outcome engine (required)
engine: 0.1.1
# user-centric title (optional)
title: {title}
# short description (optional)
desc: This is a simple scenario.
# longer description, outlining main focuses of the scenario (if there are any)
desc_long: |
    This is a simple scenario generated using endgame's init subcommand,
    template used is called 'commented'.
# author of the scenario (optional)
author: You
# website of the author (optional)
website: theoutcomeproject.com
# list of modules
mods:
- init_module: 0.1.0
# settings
#settings:
#  /uni/const/quantum_drive_tech_possible: false
"##,
            name = name.replace(" ", "_"),
            title = name.replace("_", " "),
        )
        .to_string(),
    );
    map.extend(module::template_commented(
        "init_module",
        "mods/init_module",
    ));

    map
}
// commented template
fn template_tutorial(name: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "scenario.yaml".to_string(),
        format!(
            r##"
# name should be snake_case
name: {name}
# user-centric title
title: {title}
# short description
desc: {description}
# longer description, outlining main focuses of the scenario (if there are any)
# desc_long:
# author of the scenario (optional)
# author:
# website of the author (optional)
# website:
# version of the scenario
version: 0.1.0
# version of the outcome base
outcome: 0.1.1
# list of modules
modules:
  test_module: 0.1.0
# settings, not working yet
settings:
  /uni/const/quantum_drive_tech_possible: false
"##,
            name = name,
            title = name.replace("_", " "),
            description = name.replace("_", " ") + " description",
        )
        .to_string(),
    );

    map.insert(
        "modules/init_module/module.yaml".to_string(),
        String::from(
            r##"
# name of the module, has to be snake_case without spaces (required)
name: init_module
# version of the module (required)
version: 0.1.0
# version of the outcome engine (required)
outcome: 0.2.0
# user-centric title (optional)
title: Init Module
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
        )
        .to_string(),
    );

    map
}
