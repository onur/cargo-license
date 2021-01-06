use serde_derive::Serialize;
use std::collections::{HashMap, HashSet};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

fn normalize(license_string: &str) -> String {
    let mut list: Vec<&str> = license_string
        .split('/')
        .flat_map(|e| e.split(" OR "))
        .map(str::trim)
        .collect();
    list.sort_unstable();
    list.dedup();
    list.join(" OR ")
}

#[derive(Debug, Serialize, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct DependencyDetails {
    pub name: String,
    pub version: semver::Version,
    pub authors: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub description: Option<String>,
}

impl DependencyDetails {
    #[must_use]
    pub fn new(package: &cargo_metadata::Package) -> Self {
        let authors = if package.authors.is_empty() {
            None
        } else {
            Some(package.authors.to_owned().join("|"))
        };
        Self {
            name: package.name.to_owned(),
            version: package.version.to_owned(),
            authors,
            repository: package.repository.to_owned(),
            license: package.license.as_ref().map(|s| normalize(&s)),
            license_file: package
                .license_file
                .to_owned()
                .and_then(|f| f.to_str().map(std::borrow::ToOwned::to_owned)),
            description: package
                .description
                .to_owned()
                .map(|s| s.trim().replace("\n", " ")),
        }
    }
}

pub fn get_dependencies_from_cargo_lock(
    mut metadata_command: cargo_metadata::MetadataCommand,
    avoid_dev_deps: bool,
    avoid_build_deps: bool,
) -> Result<Vec<DependencyDetails>> {
    let metadata = metadata_command.exec()?;

    let connected = {
        let resolve = metadata.resolve.as_ref().expect("missing `resolve`");

        let deps = resolve
            .nodes
            .iter()
            .map(|cargo_metadata::Node { id, deps, .. }| (id, deps))
            .collect::<HashMap<_, _>>();

        let missing_dep_kinds = deps
            .values()
            .flat_map(|d| d.iter())
            .any(|cargo_metadata::NodeDep { dep_kinds, .. }| dep_kinds.is_empty());

        if missing_dep_kinds && avoid_dev_deps {
            eprintln!("warning: Cargo 1.41+ is required for `--avoid-dev-deps`");
        }
        if missing_dep_kinds && avoid_build_deps {
            eprintln!("warning: Cargo 1.41+ is required for `--avoid-build-deps`");
        }

        let neighbors = |package_id: &cargo_metadata::PackageId| {
            deps[package_id]
                .iter()
                .filter(|cargo_metadata::NodeDep { dep_kinds, .. }| {
                    missing_dep_kinds
                        || dep_kinds
                            .iter()
                            .any(|cargo_metadata::DepKindInfo { kind, .. }| {
                                *kind == cargo_metadata::DependencyKind::Normal
                                    || !avoid_dev_deps
                                        && *kind == cargo_metadata::DependencyKind::Development
                                    || !avoid_build_deps
                                        && *kind == cargo_metadata::DependencyKind::Build
                            })
                })
                .map(|cargo_metadata::NodeDep { pkg, .. }| pkg)
        };

        let mut connected = HashSet::new();
        let stack = &mut if let Some(root) = &resolve.root {
            vec![root]
        } else {
            metadata.workspace_members.iter().collect()
        };
        while let Some(package_id) = stack.pop() {
            if connected.insert(package_id) {
                stack.extend(neighbors(package_id));
            }
        }
        connected
    };

    let mut detailed_dependencies = metadata
        .packages
        .iter()
        .filter(|p| connected.contains(&p.id))
        .map(DependencyDetails::new)
        .collect::<Vec<_>>();
    detailed_dependencies.sort_unstable();
    Ok(detailed_dependencies)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_detailed() {
        let cmd = cargo_metadata::MetadataCommand::new();
        let detailed_dependencies = get_dependencies_from_cargo_lock(cmd, false, false).unwrap();
        assert!(!detailed_dependencies.is_empty());
        for detailed_dependency in detailed_dependencies.iter() {
            assert!(
                detailed_dependency.license.is_some() || detailed_dependency.license_file.is_some()
            );
        }
    }
}
