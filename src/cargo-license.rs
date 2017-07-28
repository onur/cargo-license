
extern crate cargo_license;
extern crate ansi_term;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry::*;
use ansi_term::Colour::Green;

fn main() {
    let dependencies = cargo_license::get_dependencies_from_cargo_lock().unwrap();

    let mut table: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for dependency in dependencies {
        let license = dependency.get_license().unwrap_or("N/A".to_owned());
        match table.entry(license) {
            Vacant(e) => {e.insert(vec![dependency.name]);},
            Occupied(mut e) => {e.get_mut().push(dependency.name);},
        };
    }

    for (license, crates) in table {
        println!("{} ({}): {}",
                 Green.bold().paint(license),
                 crates.len(),
                 crates.join(", "));
    }
}
