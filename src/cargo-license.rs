
extern crate cargo_license;
extern crate ansi_term;

use std::collections::BTreeMap;
use ansi_term::Colour::Green;

fn main() {
    let dependencies = cargo_license::get_dependencies_from_cargo_lock().unwrap();

    let mut table: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for dependency in dependencies {
        let license = dependency.get_license();
        if !table.contains_key(&license) {
            table.insert(license, vec![dependency.name]);
        } else {
            table.get_mut(&license).map(|v| v.push(dependency.name.clone()));
        }
    }

    for (license, crates) in table {
        println!("{} ({}): {}",
                 Green.bold().paint(license),
                 crates.len(),
                 crates.join(", "));
    }
}
