
extern crate cargo_license;
extern crate ansi_term;

use std::collections::BTreeMap;
use ansi_term::Colour::Green;

fn main() {
    let dependencies = cargo_license::get_dependencies_from_cargo_lock().unwrap();

    let mut table: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for mut dependency in dependencies {
        let license = dependency.get_license();
        if !table.contains_key(&license) {
            table.insert(license, vec![dependency.name]);
        } else {
            table.get_mut(&license).map(|v| v.push(dependency.name.clone()));
        }
    }

    for (license, crates) in table {
        println!("{} ({}): {}", Green.bold().paint(license), crates.len(), crates.join(", "));
    }

    let mut deps = cargo_license::get_dependency_tree().unwrap();
    for incompat in cargo_license::verify_license_compatibility(&mut deps).iter() {
        println!("{} {} ({}) has an incompatible license with {} {} ({})",
                 incompat.crate1,
                 incompat.crate1_version,
                 incompat.crate1_license,
                 incompat.crate2,
                 incompat.crate2_version,
                 incompat.crate2_license);
    }
}
