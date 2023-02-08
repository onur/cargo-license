use anyhow::Result;
use cargo_metadata::{
    DepKindInfo, DependencyKind, Metadata, MetadataCommand, Node, NodeDep, Package, PackageId,
};
use itertools::Itertools;
use semver::Version;
use serde_derive::Serialize;
use std::collections::{HashMap, HashSet};
use std::io;

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

fn get_node_name_filter(metadata: &Metadata, opt: &GetDependenciesOpt) -> Result<HashSet<String>> {
    let mut filter = HashSet::new();

    let roots = if let Some(root) = metadata.root_package() {
        vec![root]
    } else {
        metadata.workspace_packages()
    };

    if opt.root_only {
        for root in roots {
            filter.insert(root.name.clone());
        }
        return Ok(filter);
    }

    if opt.direct_deps_only {
        for root in roots {
            filter.insert(root.name.clone());
            for package in root.dependencies.iter() {
                filter.insert(package.name.clone());
            }
        }
    }
    Ok(filter)
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
    pub fn new(package: &Package) -> Self {
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
            license: package.license.as_ref().map(|s| normalize(s)),
            license_file: package.license_file.to_owned().map(|f| f.into_string()),
            description: package
                .description
                .to_owned()
                .map(|s| s.trim().replace('\n', " ")),
        }
    }
}

#[derive(Debug, Serialize, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct GitlabDependency {
    name: String,
    version: Version,
    package_manager: &'static str,
    path: String,
    licenses: Vec<&'static str>,
}

#[derive(Debug, Serialize, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct GitlabLicense {
    id: &'static str,
    name: &'static str,
    url: String,
}

impl GitlabLicense {
    fn parse_licenses(dependency: &DependencyDetails) -> Result<HashSet<Self>> {
        let Some(license) = &dependency.license else {return Ok(HashSet::new())};
        let expression = spdx::Expression::parse_mode(license, spdx::ParseMode::LAX)?;
        Ok(expression
            .requirements()
            .flat_map(|req| {
                req.req.license.id().map(|license| Self {
                    id: license.name,
                    name: license.full_name,
                    url: Default::default(),
                })
            })
            .collect())
    }
}

#[derive(Debug, Serialize, Clone)]
struct GitlabLicenseScanningReport {
    version: &'static str,
    licenses: HashSet<GitlabLicense>,
    dependencies: Vec<GitlabDependency>,
}

impl TryFrom<&[DependencyDetails]> for GitlabLicenseScanningReport {
    type Error = anyhow::Error;
    fn try_from(dependencies: &[DependencyDetails]) -> Result<Self> {
        let mut licenses = HashSet::new();
        let dependencies = dependencies
            .iter()
            .cloned()
            .map(|dependency| {
                let dep_licenses = GitlabLicense::parse_licenses(&dependency)?;
                let license_ids = dep_licenses.iter().map(|license| license.id).collect();
                licenses.extend(dep_licenses);
                Ok::<_, Self::Error>(GitlabDependency {
                    name: dependency.name,
                    version: dependency.version,
                    package_manager: "cargo",
                    path: Default::default(),
                    licenses: license_ids,
                })
            })
            .try_collect()?;

        Ok(GitlabLicenseScanningReport {
            version: "2.1",
            dependencies,
            licenses,
        })
    }
}

#[derive(Default)]
pub struct GetDependenciesOpt {
    pub avoid_dev_deps: bool,
    pub avoid_build_deps: bool,
    pub direct_deps_only: bool,
    pub root_only: bool,
}

pub fn get_dependencies_from_cargo_lock(
    metadata_command: MetadataCommand,
    opt: GetDependenciesOpt,
) -> Result<Vec<DependencyDetails>> {
    let metadata = metadata_command.exec()?;

    let filter = get_node_name_filter(&metadata, &opt)?;

    let connected = {
        let resolve = metadata.resolve.as_ref().expect("missing `resolve`");

        let deps = resolve
            .nodes
            .iter()
            .map(|Node { id, deps, .. }| (id, deps))
            .collect::<HashMap<_, _>>();

        let missing_dep_kinds = deps
            .values()
            .flat_map(|d| d.iter())
            .any(|NodeDep { dep_kinds, .. }| dep_kinds.is_empty());

        if missing_dep_kinds && opt.avoid_dev_deps {
            eprintln!("warning: Cargo 1.41+ is required for `--avoid-dev-deps`");
        }
        if missing_dep_kinds && opt.avoid_build_deps {
            eprintln!("warning: Cargo 1.41+ is required for `--avoid-build-deps`");
        }

        let neighbors = |package_id: &PackageId| {
            deps[package_id]
                .iter()
                .filter(|NodeDep { dep_kinds, .. }| {
                    missing_dep_kinds
                        || dep_kinds.iter().any(|DepKindInfo { kind, .. }| {
                            *kind == DependencyKind::Normal
                                || !opt.avoid_dev_deps && *kind == DependencyKind::Development
                                || !opt.avoid_build_deps && *kind == DependencyKind::Build
                        })
                })
                .map(|NodeDep { pkg, .. }| pkg)
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
        .filter(|p| filter.is_empty() || filter.contains(&p.name))
        .map(DependencyDetails::new)
        .collect::<Vec<_>>();
    detailed_dependencies.sort_unstable();
    Ok(detailed_dependencies)
}

pub fn write_tsv(dependencies: &[DependencyDetails]) -> Result<()> {
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

pub fn write_json(dependencies: &[DependencyDetails]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&dependencies)?);
    Ok(())
}

pub fn write_gitlab(dependencies: &[DependencyDetails]) -> Result<()> {
    let dependencies = GitlabLicenseScanningReport::try_from(dependencies)?;
    println!("{}", serde_json::to_string_pretty(&dependencies)?);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_detailed() {
        let cmd = MetadataCommand::new();
        let detailed_dependencies =
            get_dependencies_from_cargo_lock(cmd, GetDependenciesOpt::default()).unwrap();
        assert!(!detailed_dependencies.is_empty());
        for detailed_dependency in detailed_dependencies.iter() {
            assert!(
                detailed_dependency.license.is_some() || detailed_dependency.license_file.is_some()
            );
        }
    }
}
