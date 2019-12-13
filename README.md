# cargo-license

[![Build Status](https://secure.travis-ci.org/onur/cargo-license.svg?branch=master)](https://travis-ci.org/onur/cargo-license)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/onur/cargo-license/master/LICENSE)

A cargo subcommand to see license of dependencies.

## Installation and Usage

You can install cargo-license with: `cargo install cargo-license` and
run it in your project directory with: `cargo license` or `cargo-license`.

```
cargo_license 0.3.0
Cargo subcommand to see licenses of dependencies.

USAGE:
    cargo-license [FLAGS] [OPTIONS]

FLAGS:
        --all-features     Activate all available features.
    -a, --authors          Display crate authors
    -d, --do-not-bundle    Output one license per line.
    -h, --help             Prints help information
    -j, --json             Detailed output as JSON.
        --no-deps          Output information only about the root package and don't fetch dependencies.
    -t, --tsv              Detailed output as tab-separated-values.
    -V, --version          Prints version information

OPTIONS:
        --current-dir <CURRENT_DIR>    Current directory of the cargo metadata process.
        --features <FEATURE>...        Space-separated list of features to activate.
        --manifest-path <PATH>         Path to Cargo.toml.
```

## Example

`cargo-license` running inside the cargo-license project directory:

![cargo-license](https://i.imgur.com/9KARkwP.png)
