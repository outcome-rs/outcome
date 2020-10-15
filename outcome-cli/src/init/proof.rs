#![allow(unused_variables)]

use std::collections::HashMap;

pub fn collect_template_files(name: &str, template_str: &str) -> Option<HashMap<String, String>> {
    match template_str {
        //"commented" => Some(template_commented(name)),
        _ => None,
    }
}

// TODO develop the templates

// commented template
fn template_commented(name: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    map.insert(
        "proof.yaml".to_string(),
        format!(
            "\
# test
"
        )
        .to_string(),
    );

    map
}
