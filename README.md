# cargo-license

[![CI](https://github.com/onur/cargo-license/workflows/CI/badge.svg)](https://github.com/onur/cargo-license/actions?workflow=CI)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/onur/cargo-license/master/LICENSE)
![Minimum Supported Rust Version](https://img.shields.io/badge/rustc-1.34-red)

A cargo subcommand to see license of dependencies.

## Installation and Usage

You can install cargo-license with: `cargo install cargo-license` and
run it in your project directory with: `cargo license` or `cargo-license`.

```
cargo-license 0.4.0
Cargo subcommand to see licenses of dependencies.

USAGE:
    cargo license [FLAGS] [OPTIONS]

FLAGS:
        --all-features        Activate all available features
    -a, --authors             Display crate authors
        --avoid-build-deps    Exclude build dependencies
        --avoid-dev-deps      Exclude development dependencies
        --direct-deps-only    Output information only about the root package and it's direct dependencies
    -d, --do-not-bundle       Output one license per line
    -h, --help                Prints help information
    -j, --json                Detailed output as JSON
        --no-default-features Deactivate default features
    -t, --tsv                 Detailed output as tab-separated-values
    -V, --version             Prints version information

OPTIONS:
        --color <WHEN>                 Coloring [possible values: auto, always, never]
        --current-dir <CURRENT_DIR>    Current directory of the cargo metadata process
        --features <FEATURE>...        Space-separated list of features to activate
        --filter-platform <TRIPLE>     Only include resolve dependencies matching the given target-triple
        --manifest-path <PATH>         Path to Cargo.toml
```

## Example

`cargo-license` running inside the cargo-license project directory:

![cargo-license](https://i.imgur.com/9KARkwP.png)
