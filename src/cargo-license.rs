extern crate ansi_term;
extern crate cargo_license;
extern crate csv;
extern crate failure;
extern crate getopts;
extern crate serde_json;

use ansi_term::Colour::Green;
use getopts::Options;
use std::collections::btree_map::Entry::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::io;
use std::process::exit;

fn group_by_license_type(
    dependencies: Vec<cargo_license::DependencyDetails>,
    display_authors: bool,
) {
    let mut table: BTreeMap<String, Vec<cargo_license::DependencyDetails>> = BTreeMap::new();

    for dependency in dependencies {
        let license = dependency
            .license
            .clone()
            .unwrap_or_else(|| "N/A".to_owned());
        match table.entry(license) {
            Vacant(e) => {
                e.insert(vec![dependency]);
            }
            Occupied(mut e) => {
                e.get_mut().push(dependency);
            }
        };
    }

    for (license, crates) in table {
        let crate_names = crates.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        if display_authors {
            let crate_authors = crates
                .iter()
                .map(|c| c.authors.clone().unwrap_or_else(|| "N/A".to_owned()))
                .collect::<BTreeSet<_>>();
            println!(
                "{} ({})\n{}\n{} {}",
                Green.bold().paint(license),
                crates.len(),
                crate_names.join(", "),
                Green.paint("by"),
                crate_authors.into_iter().collect::<Vec<_>>().join(", ")
            );
        } else {
            println!(
                "{} ({}): {}",
                Green.bold().paint(license),
                crates.len(),
                crate_names.join(", ")
            );
        }
    }
}

fn one_license_per_line(
    dependencies: Vec<cargo_license::DependencyDetails>,
    display_authors: bool,
) {
    for dependency in dependencies {
        let name = dependency.name.clone();
        let version = dependency.version.clone();
        let license = dependency.license.unwrap_or_else(|| "N/A".to_owned());
        let source = dependency.source.clone();
        if display_authors {
            let authors = dependency.authors.unwrap_or_else(|| "N/A".to_owned());
            println!(
                "{}: {}, \"{}\", {}, {} \"{}\"",
                Green.bold().paint(name),
                version,
                license,
                source,
                Green.paint("by"),
                authors
            );
        } else {
            println!(
                "{}: {}, \"{}\", {}",
                Green.bold().paint(name),
                version,
                license,
                source
            );
        }
    }
}

fn write_tsv(dependencies: &[cargo_license::DependencyDetails]) -> cargo_license::Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(io::stdout());
    for dependency in dependencies {
        wtr.serialize(dependency)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_json(dependencies: &[cargo_license::DependencyDetails]) -> cargo_license::Result<()> {
    println!("{}", serde_json::to_string_pretty(&dependencies)?);
    Ok(())
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn run() -> cargo_license::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    let program = args[0].clone();
    opts.optflag("a", "authors", "Display crate authors");
    opts.optflag("d", "do-not-bundle", "Output one license per line.");
    opts.optflag("t", "tsv", "detailed output as tab-separated-values");
    opts.optflag("j", "json", "detailed output as json");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            print_usage(&program, &opts);
            panic!(f.to_string())
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, &opts);
        return Ok(());
    }

    let display_authors = matches.opt_present("authors");
    let do_not_bundle = matches.opt_present("do-not-bundle");
    let tsv = matches.opt_present("tsv");
    let json = matches.opt_present("json");

    let dependencies = cargo_license::get_dependencies_from_cargo_lock()?;

    if tsv {
        write_tsv(&dependencies)?
    } else if json {
        write_json(&dependencies)?
    } else if do_not_bundle {
        one_license_per_line(dependencies, display_authors);
    } else {
        group_by_license_type(dependencies, display_authors);
    }
    Ok(())
}

fn main() {
    exit(match run() {
        Ok(_) => 0,
        Err(e) => {
            for cause in e.iter_chain() {
                eprintln!("{}", cause);
            }
            1
        }
    })
}
