use anyhow::Result;
use cargo_metadata::{
    DepKindInfo, DependencyKind, Metadata, MetadataCommand, Node, NodeDep, Package, PackageId,
};
use itertools::Itertools;
use semver::Version;
use serde_derive::Serialize;
use spdx::expression::ExprNode;
use spdx::LicenseReq;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::mem::swap;
use std::{io, iter};

#[derive(PartialEq, Eq, Debug)]
enum LicenseTree<'a> {
    License(&'a LicenseReq),
    Or(Vec<LicenseTree<'a>>),
    And(Vec<LicenseTree<'a>>),
}

impl PartialOrd for LicenseTree<'_> {
    // Compare based on the string representation, therefore the name of the first license
    //
    // No specific preference for AND/OR priority
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LicenseTree<'_> {
    // Compare based on the string representation, therefore the name of the first license
    //
    // No specific preference for AND/OR priority
    fn cmp(&self, other: &Self) -> Ordering {
        // let mut ordering = Ordering::Equal;
        let mut self_iter = self.license_iter();
        let mut other_iter = other.license_iter();

        loop {
            let left = self_iter.next();
            let right = other_iter.next();

            match (left, right) {
                (None, None) => return Ordering::Equal,
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (Some(l), Some(r)) => match l.cmp(r) {
                    Ordering::Equal => {}
                    Ordering::Less => return Ordering::Less,
                    Ordering::Greater => return Ordering::Greater,
                },
            }
        }
    }

    pub fn normalize(&mut self) {
        match self {
            Self::License(_) => {}
            Self::Or(nodes) => {
                let mut acc = Vec::new();
                for v in nodes.iter_mut() {
                    v.normalize();
                }
                nodes.retain_mut(|v| {
                    if let Self::Or(ref mut v) = v {
                        acc.append(v);
                        false
                    } else {
                        true
                    }
                });
                nodes.append(&mut acc);
                nodes.sort_by(Self::cmp);
            }
            Self::And(nodes) => {
                let mut acc = Vec::new();
                for v in nodes.iter_mut() {
                    v.normalize();
                }
                nodes.retain_mut(|v| {
                    if let Self::And(ref mut v) = v {
                        acc.append(v);
                        false
                    } else {
                        true
                    }
                });
                nodes.append(&mut acc);
                nodes.sort_by(Self::cmp);
            }
        }
    }

    fn license_iter(&self) -> Box<dyn Iterator<Item = &LicenseReq> + '_> {
        match self {
            Self::License(l) => Box::new(iter::once(*l)),
            Self::Or(nodes) | Self::And(nodes) => {
                Box::new(nodes.iter().flat_map(LicenseTree::license_iter))
            }
        }
    }

    fn serialize(&self) -> String {
        let mut acc = String::new();
        self.serialize_inner(&mut acc, false);
        acc
    }

    fn serialize_inner(&self, acc: &mut String, is_first: bool) {
        match self {
            Self::License(l) => acc.push_str(&l.to_string()),
            Self::Or(v) => {
                let need_parentheses = v.len() != 1 && is_first;
                if need_parentheses {
                    acc.push('(');
                }

                let mut v_iter = v.iter().peekable();
                while let Some(node) = v_iter.next() {
                    node.serialize_inner(acc, true);
                    if v_iter.peek().is_some() {
                        acc.push_str(" OR ");
                    }
                }
                if need_parentheses {
                    acc.push(')');
                }
            }
            Self::And(v) => {
                let need_parentheses = v.len() != 1 && is_first;
                if need_parentheses {
                    acc.push('(');
                }

                let mut v_iter = v.iter().peekable();
                while let Some(node) = v_iter.next() {
                    node.serialize_inner(acc, true);
                    if v_iter.peek().is_some() {
                        acc.push_str(" AND ");
                    }
                }
                if need_parentheses {
                    acc.push(')');
                }
            }
        }
    }
}

#[must_use]
pub fn normalize(license_string: &str) -> String {
    let canon = spdx::Expression::canonicalize(license_string).unwrap_or_default();

    let Ok(license) = spdx::Expression::parse_mode(
        canon.as_deref().unwrap_or(license_string),
        spdx::ParseMode::LAX,
    ) else {
        return license_string.into();
    };

    let mut req_stack = Vec::new();
    let _: Vec<_> = license.iter().collect();
    for op in license.iter() {
        match op {
            ExprNode::Req(r) => req_stack.push(LicenseTree::License(&r.req)),
            ExprNode::Op(spdx::expression::Operator::Or) => {
                let Some(mut left) = req_stack.pop() else {
                    return license_string.into();
                };
                let Some(mut right) = req_stack.pop() else {
                    return license_string.into();
                };

                // Order elements here based on the name of the first license that appears
                if left > right {
                    swap(&mut left, &mut right);
                }

                req_stack.push(LicenseTree::Or(vec![left, right]));
            }
            ExprNode::Op(spdx::expression::Operator::And) => {
                let Some(mut left) = req_stack.pop() else {
                    return license_string.into();
                };
                let Some(mut right) = req_stack.pop() else {
                    return license_string.into();
                };

                // Order elements here based on the name of the first license that appears
                if left > right {
                    swap(&mut left, &mut right);
                }

                req_stack.push(LicenseTree::And(vec![left, right]));
            }
        }
    }

    let [ref mut tree] = &mut *req_stack else {
        return license_string.into();
    };

    tree.normalize();
    tree.serialize()
}

fn get_proc_macro_node_names(metadata: &Metadata, opt: &GetDependenciesOpt) -> HashSet<String> {
    let mut proc_macros = HashSet::new();
    if opt.avoid_proc_macros {
        for packages in &metadata.packages {
            for target in &packages.targets {
                if target.crate_types.contains(&String::from("proc-macro")) {
                    proc_macros.insert(target.name.clone());
                    for package in &packages.dependencies {
                        proc_macros.insert(package.name.clone());
                    }
                }
            }
        }
    }
    proc_macros
}

fn get_node_name_filter(metadata: &Metadata, opt: &GetDependenciesOpt) -> HashSet<String> {
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
        return filter;
    }

    if opt.direct_deps_only {
        for root in roots {
            filter.insert(root.name.clone());
            for package in &root.dependencies {
                filter.insert(package.name.clone());
            }
        }
    }
    filter
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
            Some(package.authors.clone().join("|"))
        };
        Self {
            name: package.name.clone(),
            version: package.version.clone(),
            authors,
            repository: package.repository.clone(),
            license: package.license.as_ref().map(|s| normalize(s)),
            license_file: package
                .license_file
                .clone()
                .map(cargo_metadata::camino::Utf8PathBuf::into_string),
            description: package
                .description
                .clone()
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
        let Some(license) = &dependency.license else {
            return Ok(HashSet::new());
        };
        let expression = spdx::Expression::parse_mode(license, spdx::ParseMode::LAX)?;
        Ok(expression
            .requirements()
            .filter_map(|req| {
                req.req.license.id().map(|license| Self {
                    id: license.name,
                    name: license.full_name,
                    url: String::default(),
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
                    path: String::default(),
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

// This is using bools as flags and all combinations are fine
// It is not a state machine
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct GetDependenciesOpt {
    pub avoid_dev_deps: bool,
    pub avoid_build_deps: bool,
    pub avoid_proc_macros: bool,
    pub direct_deps_only: bool,
    pub root_only: bool,
}

/// Get the list of dependencies from the Cargo.lock
///
/// # Errors
///
/// Will error if running the metadata command fails
// Can't panic in normal operation
#[allow(clippy::missing_panics_doc)]
pub fn get_dependencies_from_cargo_lock(
    metadata_command: &MetadataCommand,
    opt: &GetDependenciesOpt,
) -> Result<Vec<DependencyDetails>> {
    let metadata = metadata_command.exec()?;

    let node_name_filter = get_node_name_filter(&metadata, opt);
    let proc_macro_exclusions = get_proc_macro_node_names(&metadata, opt);

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
        .filter(|p| node_name_filter.is_empty() || node_name_filter.contains(&p.name))
        .filter(|p| !proc_macro_exclusions.contains(&p.name))
        .map(DependencyDetails::new)
        .collect::<Vec<_>>();
    detailed_dependencies.sort_unstable();
    Ok(detailed_dependencies)
}

/// Write the dependency information in a tab-separated format to the output writer.
///
/// # Errors
///
/// Will error if output writer is closed
pub fn write_tsv(
    dependencies: &[DependencyDetails],
    output_writer: Box<dyn io::Write>,
) -> Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(output_writer);
    for dependency in dependencies {
        wtr.serialize(dependency)?;
    }
    wtr.flush()?;
    Ok(())
}

/// Write the dependency information in JSON format to output writer
///
/// # Errors
///
/// Will error if output writer is closed
pub fn write_json(
    dependencies: &[DependencyDetails],
    output_writer: &mut Box<dyn io::Write>,
) -> Result<()> {
    writeln!(
        output_writer,
        "{}",
        serde_json::to_string_pretty(&dependencies)?
    )?;
    Ok(())
}

/// Write the dependency information in the Gitlab license scanning format to output writer
///
/// # Errors
///
/// Will error if output writer is closed
pub fn write_gitlab(
    dependencies: &[DependencyDetails],
    output_writer: &mut Box<dyn io::Write>,
) -> Result<()> {
    let dependencies = GitlabLicenseScanningReport::try_from(dependencies)?;
    writeln!(
        output_writer,
        "{}",
        serde_json::to_string_pretty(&dependencies)?
    )?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_detailed() {
        let cmd = MetadataCommand::new();
        let detailed_dependencies =
            get_dependencies_from_cargo_lock(&cmd, &GetDependenciesOpt::default()).unwrap();
        assert!(!detailed_dependencies.is_empty());
        for detailed_dependency in &detailed_dependencies {
            assert!(
                detailed_dependency.license.is_some() || detailed_dependency.license_file.is_some()
            );
        }
    }

    #[test]
    fn test_normalize() {
        let tests = [
            ("MIT", "MIT"),
            ("MIT OR Apache-2.0", "Apache-2.0 OR MIT"),
            ("Apache-2.0 OR MIT", "Apache-2.0 OR MIT"),
            (
                "(Apache-2.0 OR MIT) AND Apache-2.0",
                "(Apache-2.0 OR MIT) AND Apache-2.0",
            ),
            ("(Apache-2.0 OR MIT) AND MIT", "(Apache-2.0 OR MIT) AND MIT"),
            (
                "Apache-2.0 AND (Apache-2.0 OR MIT)",
                "(Apache-2.0 OR MIT) AND Apache-2.0",
            ),
            ("MIT AND (Apache-2.0 OR MIT)", "(Apache-2.0 OR MIT) AND MIT"),
            (
                "(Apache-2.0 AND Unicode-3.0) OR (Apache-2.0 AND MIT)",
                "(Apache-2.0 AND MIT) OR (Apache-2.0 AND Unicode-3.0)",
            ),
            (
                "Unicode-3.0 AND (MIT OR Apache-2.0)",
                "(Apache-2.0 OR MIT) AND Unicode-3.0",
            ),
            (
                "(MIT OR Apache-2.0) AND Unicode-3.0",
                "(Apache-2.0 OR MIT) AND Unicode-3.0",
            ),
            ("MIT AND (MIT OR Apache-2.0)", "(Apache-2.0 OR MIT) AND MIT"),
            (
                "((((Apache-2.0 WITH LLVM-exception) OR (Apache-2.0)) AND (OpenSSL)) OR (MIT))",
                "((Apache-2.0 OR Apache-2.0 WITH LLVM-exception) AND OpenSSL) OR MIT",
            ),
            (
                "Borceux OR MIT AND BitTorrent-1.1",
                "(BitTorrent-1.1 AND MIT) OR Borceux",
            ),
            ("Zlib OR Apache-2.0 OR MIT", "Apache-2.0 OR MIT OR Zlib"),
            ("MIT OR Zlib OR Apache-2.0", "Apache-2.0 OR MIT OR Zlib"),
            (
                "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
                "Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT",
            ),
        ];

        for (i, o) in tests {
            assert_eq!(normalize(i), o, "Input {i}");
        }
    }
}
