#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use ansi_term::Colour::Green;
use ansi_term::Style;
use clap::Parser;
use std::borrow::Cow;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::process::exit;

fn group_by_license_type(
    dependencies: Vec<cargo_license::DependencyDetails>,
    display_authors: bool,
    enable_color: bool,
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
                colored(&license, &Green.bold(), enable_color),
                crates.len(),
                crate_names.join(", "),
                colored("by", &Green.normal(), enable_color),
                crate_authors.into_iter().collect::<Vec<_>>().join(", ")
            );
        } else {
            println!(
                "{} ({}): {}",
                colored(&license, &Green.bold(), enable_color),
                crates.len(),
                crate_names.join(", ")
            );
        }
    }
}

fn one_license_per_line(
    dependencies: Vec<cargo_license::DependencyDetails>,
    display_authors: bool,
    enable_color: bool,
) {
    for dependency in dependencies {
        let name = dependency.name.clone();
        let version = dependency.version.clone();
        let license = dependency.license.unwrap_or_else(|| "N/A".to_owned());
        if display_authors {
            let authors = dependency.authors.unwrap_or_else(|| "N/A".to_owned());
            println!(
                "{}: {}, \"{}\", {}, \"{}\"",
                colored(&name, &Green.bold(), enable_color),
                version,
                license,
                colored("by", &Green.normal(), enable_color),
                authors
            );
        } else {
            println!(
                "{}: {}, \"{}\",",
                colored(&name, &Green.bold(), enable_color),
                version,
                license,
            );
        }
    }
}

fn colored<'a, 'b>(s: &'a str, style: &'b Style, enable_color: bool) -> Cow<'a, str> {
    if enable_color {
        Cow::Owned(format!("{}", style.paint(s)))
    } else {
        Cow::Borrowed(s)
    }
}

fn write_xsv(dependencies: &[cargo_license::DependencyDetails],delimiter: u8 ) -> cargo_license::Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
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

#[derive(Debug, Parser)]
#[allow(clippy::struct_excessive_bools)]
#[clap(
    bin_name = "cargo license",
    about = "Cargo subcommand to see licenses of dependencies."
)]
struct Opt {
    #[clap(value_name = "PATH", long)]
    /// Path to Cargo.toml.
    manifest_path: Option<PathBuf>,

    #[clap(value_name = "CURRENT_DIR", long)]
    /// Current directory of the cargo metadata process.
    current_dir: Option<PathBuf>,

    #[clap(short, long)]
    /// Display crate authors
    authors: bool,

    #[clap(short, long)]
    /// Output one license per line.
    do_not_bundle: bool,

    #[clap(short, long)]
    /// Detailed output as tab-separated-values.
    tsv: bool,

    #[clap(short, long)]
    /// Detailed output as comma-separated-values.
    csv: bool,

    #[clap(short, long)]
    /// Detailed output as JSON.
    json: bool,

    #[clap(long)]
    /// Exclude development dependencies
    avoid_dev_deps: bool,

    #[clap(long)]
    /// Exclude build dependencies
    avoid_build_deps: bool,

    #[clap(long = "features", value_name = "FEATURE")]
    /// Space-separated list of features to activate.
    features: Option<Vec<String>>,

    #[clap(long = "all-features")]
    /// Activate all available features.
    all_features: bool,

    #[clap(long = "no-deps")]
    /// Output information only about the root package and don't fetch dependencies.
    no_deps: bool,

    #[clap(long = "filter-platform", value_name = "TRIPLE")]
    /// Only include resolve dependencies matching the given target-triple.
    filter_platform: Option<String>,

    #[clap(
        long = "color",
        name = "WHEN",
        possible_value = "auto",
        possible_value = "always",
        possible_value = "never"
    )]
    /// Coloring
    color: Option<String>,
}

fn run() -> cargo_license::Result<()> {
    use std::env;

    // Drop extra `license` argument when called by `cargo`.
    let args = env::args().enumerate().filter_map(|(i, x)| {
        if (i, x.as_str()) == (1, "license") {
            None
        } else {
            Some(x)
        }
    });

    let opt = Opt::parse_from(args);
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
    if let Some(triple) = opt.filter_platform {
        cmd.other_options(["--filter-platform".into(), triple]);
    }

    let dependencies = cargo_license::get_dependencies_from_cargo_lock(
        cmd,
        opt.avoid_dev_deps,
        opt.avoid_build_deps,
    )?;

    let enable_color = if let Some(color) = opt.color {
        match color.as_ref() {
            "auto" => atty::is(atty::Stream::Stdout),
            "always" => true,
            "never" => false,
            _ => unreachable!(),
        }
    } else {
        atty::is(atty::Stream::Stdout)
    };

    if opt.tsv {
        write_xsv(&dependencies,b'\t')?;
    }else if opt.csv{
        write_xsv(&dependencies,b',')?;
    } else if opt.json {
        write_json(&dependencies)?;
    } else if opt.do_not_bundle {
        one_license_per_line(dependencies, opt.authors, enable_color);
    } else {
        group_by_license_type(dependencies, opt.authors, enable_color);
    }
    Ok(())
}

fn main() {
    exit(match run() {
        Ok(_) => 0,
        Err(e) => {
            for cause in e.chain() {
                eprintln!("{}", cause);
            }
            1
        }
    })
}
