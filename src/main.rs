#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use anstyle::AnsiColor::Green;
use anstyle::Style;
use anyhow::Result;
use cargo_license::{
    get_dependencies_from_cargo_lock, write_gitlab, write_json, write_tsv, DependencyDetails,
    GetDependenciesOpt,
};
use cargo_metadata::{CargoOpt, MetadataCommand};
use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::{Parser, ValueEnum};
use std::borrow::Cow;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::exit;

fn group_by_license_type(
    dependencies: Vec<DependencyDetails>,
    display_authors: bool,
    enable_color: bool,
    output_writer: &mut Box<dyn Write>,
) {
    let mut table: BTreeMap<String, Vec<DependencyDetails>> = BTreeMap::new();

    for dependency in dependencies {
        let license_file = dependency.license_file.as_ref();
        let license = dependency.license.clone().unwrap_or_else(move || {
            if license_file.is_some() {
                "Custom License File".to_owned()
            } else {
                "N/A".to_owned()
            }
        });
        match table.entry(license) {
            Vacant(e) => {
                e.insert(vec![dependency]);
            }
            Occupied(mut e) => {
                e.get_mut().push(dependency);
            }
        }
    }

    for (license, crates) in table {
        let crate_names = crates.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        if display_authors {
            let crate_authors = crates
                .iter()
                .map(|c| c.authors.clone().unwrap_or_else(|| "N/A".to_owned()))
                .collect::<BTreeSet<_>>();
            writeln!(
                output_writer,
                "{} ({})\n{}\n{} {}",
                colored(
                    &license,
                    &Style::new().fg_color(Some(Green.into())).bold(),
                    enable_color
                ),
                crates.len(),
                crate_names.join(", "),
                colored(
                    "by",
                    &Style::new().fg_color(Some(Green.into())),
                    enable_color
                ),
                crate_authors.into_iter().collect::<Vec<_>>().join(", ")
            )
            .unwrap();
        } else {
            writeln!(
                output_writer,
                "{} ({}): {}",
                colored(
                    &license,
                    &Style::new().fg_color(Some(Green.into())).bold(),
                    enable_color
                ),
                crates.len(),
                crate_names.join(", ")
            )
            .unwrap();
        }
    }
}

fn one_license_per_line(
    dependencies: Vec<DependencyDetails>,
    display_authors: bool,
    enable_color: bool,
    output_writer: &mut Box<dyn Write>,
) {
    for dependency in dependencies {
        let name = dependency.name.clone();
        let version = dependency.version.clone();
        let license_file = dependency.license_file.as_ref();
        let license = dependency.license.unwrap_or_else(move || {
            if license_file.is_some() {
                "Custom License File".to_owned()
            } else {
                "N/A".to_owned()
            }
        });
        if display_authors {
            let authors = dependency.authors.unwrap_or_else(|| "N/A".to_owned());
            writeln!(
                output_writer,
                "{}: {}, \"{}\", {}, \"{}\"",
                colored(
                    &name,
                    &Style::new().fg_color(Some(Green.into())).bold(),
                    enable_color
                ),
                version,
                license,
                colored(
                    "by",
                    &Style::new().fg_color(Some(Green.into())),
                    enable_color
                ),
                authors
            )
            .unwrap();
        } else {
            writeln!(
                output_writer,
                "{}: {}, \"{}\",",
                colored(
                    &name,
                    &Style::new().fg_color(Some(Green.into())).bold(),
                    enable_color
                ),
                version,
                license,
            )
            .unwrap();
        }
    }
}

fn colored<'a>(s: &'a str, style: &Style, enable_color: bool) -> Cow<'a, str> {
    if enable_color {
        Cow::Owned(format!("{style}{s}{style:#}"))
    } else {
        Cow::Borrowed(s)
    }
}

#[derive(Debug, Parser)]
#[allow(clippy::struct_excessive_bools)]
#[clap(
    bin_name = "cargo license",
    about = "Cargo subcommand to see licenses of dependencies."
)]
#[clap(
    styles(Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Yellow.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
    )
)]
struct Opt {
    #[clap(value_name = "PATH", long, display_order(0))]
    /// Path to Cargo.toml.
    manifest_path: Option<PathBuf>,

    #[clap(value_name = "CURRENT_DIR", long, display_order(0))]
    /// Current directory of the cargo metadata process.
    current_dir: Option<PathBuf>,

    #[clap(short, long, display_order(0))]
    /// Display crate authors
    authors: bool,

    #[clap(short, long, display_order(0))]
    /// Output one license per line.
    do_not_bundle: bool,

    #[clap(short, long, display_order(0))]
    /// Detailed output as tab-separated-values.
    tsv: bool,

    #[clap(short, long, display_order(0))]
    /// Detailed output as JSON.
    json: bool,

    #[clap(short, long, display_order(0))]
    /// Gitlab license scanner output
    gitlab: bool,

    #[clap(value_name = "PATH", short, long, display_order(0))]
    /// Output to file
    output: Option<PathBuf>,

    #[clap(long, display_order(0))]
    /// Exclude development dependencies
    avoid_dev_deps: bool,

    #[clap(long, display_order(0))]
    /// Exclude build dependencies
    avoid_build_deps: bool,

    #[clap(long, display_order(0))]
    /// Exclude `proc_macros` dependencies
    avoid_proc_macros: bool,

    #[clap(long = "features", value_name = "FEATURE", display_order(0))]
    /// Space-separated list of features to activate.
    features: Option<Vec<String>>,

    #[clap(long = "all-features", display_order(0))]
    /// Activate all available features.
    all_features: bool,

    #[clap(long = "no-default-features", display_order(0))]
    /// Deactivate default features
    no_default_features: bool,

    #[clap(long = "direct-deps-only", display_order(0))]
    /// Output information only about the root package and don't fetch dependencies.
    direct_deps_only: bool,

    #[clap(long = "root-only", display_order(0))]
    /// Output information only about the root package.
    root_only: bool,

    #[clap(long = "filter-platform", value_name = "TRIPLE", display_order(0))]
    /// Only include resolve dependencies matching the given target-triple.
    filter_platform: Option<String>,

    #[clap(
        long = "color",
        name = "WHEN",
        value_enum,
        default_value = "auto",
        display_order(0)
    )]
    /// Coloring
    color: Color,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug)]
enum Color {
    Auto,
    Always,
    Never,
}

fn run() -> Result<()> {
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
    let mut cmd = MetadataCommand::new();

    if let Some(path) = &opt.manifest_path {
        cmd.manifest_path(path);
    }
    if let Some(dir) = &opt.current_dir {
        cmd.current_dir(dir);
    }
    if opt.all_features {
        cmd.features(CargoOpt::AllFeatures);
    }
    if opt.no_default_features {
        cmd.features(CargoOpt::NoDefaultFeatures);
    }
    if let Some(features) = opt.features {
        cmd.features(CargoOpt::SomeFeatures(features));
    }
    if let Some(triple) = opt.filter_platform {
        cmd.other_options(["--filter-platform".into(), triple]);
    }

    let get_opts = GetDependenciesOpt {
        avoid_dev_deps: opt.avoid_dev_deps,
        avoid_build_deps: opt.avoid_build_deps,
        avoid_proc_macros: opt.avoid_proc_macros,
        direct_deps_only: opt.direct_deps_only,
        root_only: opt.root_only,
    };

    let dependencies = get_dependencies_from_cargo_lock(&cmd, &get_opts)?;

    let enable_color = match opt.color {
        Color::Auto => io::stdin().is_terminal(),
        Color::Always => true,
        Color::Never => false,
    };

    let mut output_writer = match opt.output {
        Some(o) => Box::new(File::create(o)?) as Box<dyn Write>,
        None => Box::new(io::stdout()) as Box<dyn Write>,
    };

    if opt.tsv {
        write_tsv(&dependencies, output_writer)?;
    } else if opt.json {
        write_json(&dependencies, &mut output_writer)?;
    } else if opt.gitlab {
        write_gitlab(&dependencies, &mut output_writer)?;
    } else if opt.do_not_bundle {
        one_license_per_line(dependencies, opt.authors, enable_color, &mut output_writer);
    } else {
        group_by_license_type(dependencies, opt.authors, enable_color, &mut output_writer);
    }
    Ok(())
}

fn main() {
    exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            for cause in e.chain() {
                eprintln!("{cause}");
            }
            1
        }
    })
}
