use serde_derive::Serialize;

pub type Result<T> = std::result::Result<T, failure::Error>;

fn normalize(license_string: &str) -> String {
    let mut list: Vec<&str> = license_string
        .split('/')
        .flat_map(|e| e.split(" OR "))
        .map(str::trim)
        .collect();
    list.sort();
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
) -> Result<Vec<DependencyDetails>> {
    let metadata = metadata_command.exec()?;

    let mut detailed_dependencies: Vec<DependencyDetails> = Vec::new();
    for package in metadata.packages {
        detailed_dependencies.push(DependencyDetails::new(&package));
    }
    detailed_dependencies.sort_by(|dep1, dep2| dep1.cmp(dep2));
    Ok(detailed_dependencies)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_detailed() {
        let cmd = cargo_metadata::MetadataCommand::new();
        let detailed_dependencies = get_dependencies_from_cargo_lock(cmd).unwrap();
        assert!(!detailed_dependencies.is_empty());
        for detailed_dependency in detailed_dependencies.iter() {
            assert!(
                detailed_dependency.license.is_some() || detailed_dependency.license_file.is_some()
            );
        }
    }
}
