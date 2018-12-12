extern crate cargo;
#[macro_use]
extern crate failure;
extern crate toml;
#[macro_use]
extern crate serde_derive;

use cargo::core;
use cargo::sources::SourceConfigMap;
use cargo::util::{errors::internal, Config};
use std::fs;

pub type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Debug, Fail)]
pub enum LicenseError {
    #[fail(display = "invalid configuration: {}", _0)]
    InvalidConfiguration(String),
}

fn normalize(license_string: &Option<String>) -> Option<String> {
    match license_string {
        None => None,
        Some(ref license) => {
            let mut list: Vec<&str> = license.split('/').collect();
            for elem in list.iter_mut() {
                *elem = elem.trim();
            }
            list.sort();
            list.dedup();
            Some(list.join("|"))
        }
    }
}

#[derive(Debug, Serialize, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct DependencyDetails {
    pub name: String,
    pub version: String,
    pub source: String,
    pub authors: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository_tree: Option<String>,
    pub homepage: Option<String>,
    pub description: Option<String>,
}

impl DependencyDetails {
    pub fn load(name: &str, version: &str, source: &str) -> Result<Vec<DependencyDetails>> {
        // TODO: crates-license is only working for crates.io registry
        if !source.starts_with("registry") {
            Err(internal("registry sources are unimplemented"))?;
        }

        let config = Config::default()?;
        let source_id = core::SourceId::from_url(source)?;
        let source_map = SourceConfigMap::new(&config)?;
        let mut source_cfg = source_map.load(&source_id)?;
        // update crates.io-index registry
        source_cfg.update()?;

        let primary_dependency: core::Dependency =
            core::Dependency::parse_no_deprecated(name, Some(version), &source_id)?;
        let summaries = source_cfg.query_vec(&primary_dependency)?;
        let mut dependencies: Vec<DependencyDetails> = Vec::new();

        for summery in summaries.iter() {
            let pkg_id = summery.package_id();
            let package = source_cfg.download(pkg_id)?;
            let manifest_metadata = package.manifest().metadata();
            let version = format!("{}", package.version());
            let repo = &manifest_metadata.repository;

            let repository_tree = match repo {
                Some(r) => {
                    if r.contains("github") || r.contains("gitlab") {
                        Some(format!("{}/tree/v{}", r.to_owned(), version))
                    } else {
                        None
                    }
                }
                None => None,
            };

            dependencies.push(DependencyDetails {
                name: package.name().as_str().into(),
                version,
                source: source.to_owned(),
                authors: Some(package.authors().to_owned().join("|")),
                repository: repo.to_owned(),
                license: normalize(&manifest_metadata.license.to_owned()),
                license_file: manifest_metadata.license_file.to_owned(),
                repository_tree,
                homepage: manifest_metadata.homepage.to_owned(),
                description: manifest_metadata
                    .description
                    .to_owned()
                    .map(|s| s.trim().replace("\n", " ")),
            });
        }
        Ok(dependencies)
    }
}

pub fn get_dependencies_from_cargo_lock() -> Result<Vec<DependencyDetails>> {
    let cargo_toml_value = fs::read_to_string("Cargo.toml")?.parse::<toml::Value>()?;
    let cargo_toml = cargo_toml_value.as_table().ok_or_else(|| {
        LicenseError::InvalidConfiguration("Unexpected format in Cargo.toml".into())
    })?;

    let cargo_lock_value = fs::read_to_string("Cargo.lock")?.parse::<toml::Value>()?;
    let cargo_lock = cargo_lock_value.as_table().ok_or_else(|| {
        LicenseError::InvalidConfiguration("Unexpected format in Cargo.lock".into())
    })?;

    let root_project = cargo_toml
        .get("package")
        .and_then(|x| x.get("name"))
        .ok_or_else(|| {
            LicenseError::InvalidConfiguration(
                "Could not identify name of project in Cargo.toml".into(),
            )
        })?
        .as_str()
        .to_owned();

    let packages = cargo_lock
        .get("package")
        .and_then(|p| p.as_array())
        .ok_or_else(|| {
            LicenseError::InvalidConfiguration("\"package\" not found in cargo.lock".into())
        })?;

    let mut detailed_dependencies: Vec<DependencyDetails> = Vec::new();

    for package in packages.iter() {
        let p_table = package.as_table();
        let name = p_table
            .and_then(|n| n.get("name"))
            .and_then(|n| n.as_str())
            .to_owned()
            .ok_or_else(|| {
                LicenseError::InvalidConfiguration(
                    "Could not identify name of dependency in Cargo.toml".into(),
                )
            })?;
        let version = p_table
            .and_then(|n| n.get("version"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| {
                LicenseError::InvalidConfiguration(
                    "Could not identify version of dependency in Cargo.toml".into(),
                )
            })?;
        // The source is empty for the source crate.
        // It can be exclude it since it isn't a dependency.
        let source = p_table
            .and_then(|n| n.get("source"))
            .and_then(|n| n.as_str())
            .unwrap_or("");

        if source.is_empty() && Some(name) == root_project {
            continue;
        } else {
            detailed_dependencies.append(&mut DependencyDetails::load(name, version, source)?);
        }
    }
    Ok(detailed_dependencies)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_detailed() {
        let detailed_dependencies = get_dependencies_from_cargo_lock().unwrap();
        assert!(!detailed_dependencies.is_empty());
        for detailed_dependency in detailed_dependencies.iter() {
            assert!(
                detailed_dependency.license.is_some() || detailed_dependency.license_file.is_some()
            );
        }
    }
}
