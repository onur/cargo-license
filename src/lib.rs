extern crate cargo_metadata;
extern crate failure;
extern crate toml;
#[macro_use]
extern crate serde_derive;

pub type Result<T> = std::result::Result<T, failure::Error>;

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
    pub authors: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub description: Option<String>,
}

impl DependencyDetails {
    pub fn new(package: &cargo_metadata::Package) -> Self {
        let authors = if package.authors.is_empty() {
            None
        } else {
            Some(package.authors.to_owned().join("|"))
        };
        DependencyDetails {
            name: package.name.to_owned(),
            version: package.version.to_owned(),
            authors,
            repository: package.repository.to_owned(),
            license: normalize(&package.license.to_owned()),
            license_file: package
                .license_file
                .to_owned()
                .and_then(|f| f.to_str().map(|x| x.to_owned())),
            description: package
                .description
                .to_owned()
                .map(|s| s.trim().replace("\n", " ")),
        }
    }
}

pub fn get_dependencies_from_cargo_lock() -> Result<Vec<DependencyDetails>> {
    let mut path = std::env::current_dir()?;
    path.push("Cargo.toml");
    let metadata =
        cargo_metadata::metadata_deps(Some(&path), true).map_err(failure::SyncFailure::new)?;

    let mut detailed_dependencies: Vec<DependencyDetails> = Vec::new();
    for package in metadata.packages {
        detailed_dependencies.push(DependencyDetails::new(&package));
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
