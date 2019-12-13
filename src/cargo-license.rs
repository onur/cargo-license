extern crate ansi_term;
extern crate cargo_license;
extern crate csv;
extern crate failure;
extern crate getopts;
extern crate serde_json;

use ansi_term::Colour::Green;
use std::collections::btree_map::Entry::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::PathBuf;
use std::process::exit;
use structopt::StructOpt;

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
        if display_authors {
            let authors = dependency.authors.unwrap_or_else(|| "N/A".to_owned());
            println!(
                "{}: {}, \"{}\", {}, \"{}\"",
                Green.bold().paint(name),
                version,
                license,
                Green.paint("by"),
                authors
            );
        } else {
            println!(
                "{}: {}, \"{}\",",
                Green.bold().paint(name),
                version,
                license,
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

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cargo_license",
    about = "Cargo subcommand to see licenses of dependencies."
)]
struct Opt {
    #[structopt(name = "PATH", long = "manifest-path", parse(from_os_str))]
    /// Path to Cargo.toml.
    manifest_path: Option<PathBuf>,

    #[structopt(name = "CURRENT_DIR", long = "current-dir", parse(from_os_str))]
    /// Current directory of the cargo metadata process.
    current_dir: Option<PathBuf>,

    #[structopt(short, long)]
    /// Display crate authors
    authors: bool,

    #[structopt(short, long)]
    /// Output one license per line.
    do_not_bundle: bool,

    #[structopt(short, long)]
    /// Detailed output as tab-separated-values.
    tsv: bool,

    #[structopt(short, long)]
    /// Detailed output as JSON.
    json: bool,

    #[structopt(long = "features", name = "FEATURE")]
    /// Space-separated list of features to activate.
    features: Option<Vec<String>>,

    #[structopt(long = "all-features")]
    /// Activate all available features.
    all_features: bool,

    #[structopt(long = "no-deps")]
    /// Output information only about the root package and don't fetch dependencies.
    no_deps: bool,
}

fn run() -> cargo_license::Result<()> {
    let opt = Opt::from_args();
    let mut cmd = cargo_metadata::MetadataCommand::new();

    if let Some(path) = &opt.manifest_path {
        cmd.manifest_path(path);
    }
    if let Some(dir) = &opt.current_dir {
        cmd.current_dir(dir);
    }
    if opt.all_features {
        cmd.features(cargo_metadata::CargoOpt::AllFeatures);
    }
    if opt.no_deps {
        cmd.features(cargo_metadata::CargoOpt::NoDefaultFeatures);
    }
    if let Some(features) = opt.features {
        cmd.features(cargo_metadata::CargoOpt::SomeFeatures(features));
    }

    let dependencies = cargo_license::get_dependencies_from_cargo_lock(cmd)?;

    if opt.tsv {
        write_tsv(&dependencies)?
    } else if opt.json {
        write_json(&dependencies)?
    } else if opt.do_not_bundle {
        one_license_per_line(dependencies, opt.authors);
    } else {
        group_by_license_type(dependencies, opt.authors);
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
