
extern crate cargo_license;
extern crate ansi_term;
extern crate getopts;

use std::env;
use getopts::Options;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry::*;
use ansi_term::Colour::Green;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.optflag("", "authors", "Display crate authors");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(err) => {
            println!("Usage: {} [--authors]\n{}", args[0], err);
            std::process::exit(1);
        }
    };

    let display_authors = matches.opt_present("authors");

    let dependencies = cargo_license::get_dependencies_from_cargo_lock().unwrap();

    let mut table: BTreeMap<String, Vec<cargo_license::Dependency>> = BTreeMap::new();

    for dependency in dependencies {
        let license = dependency.get_license().unwrap_or("N/A".to_owned());
        match table.entry(license) {
            Vacant(e) => {e.insert(vec![dependency]);},
            Occupied(mut e) => {e.get_mut().push(dependency);},
        };
    }

    for (license, crates) in table {
        let crate_names = crates.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        if display_authors {
            let crate_authors = crates.iter().flat_map(|c| c.get_authors().unwrap_or(vec![])).collect::<BTreeSet<_>>();
            println!("{} ({})\n{}\n{} {}",
                     Green.bold().paint(license),
                     crates.len(),
                     crate_names.join(", "),
                     Green.paint("by"),
                     crate_authors.into_iter().collect::<Vec<_>>().join(", "));
        } else {
            println!("{} ({}): {}",
                     Green.bold().paint(license),
                     crates.len(),
                     crate_names.join(", "));
        }
    }
}
