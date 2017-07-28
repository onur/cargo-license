

extern crate cargo;
extern crate toml;
#[macro_use]
extern crate error_chain;

use std::io;
use cargo::util::CargoResult;

// I thought this crate is a good example to learn error_chain
// but looks like no need of it in this crate
error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links {}

    foreign_links {
        Io(io::Error);
    }

    errors {}
}

#[derive(Debug, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub source: String,
}


impl Dependency {
    fn get_cargo_package(&self) -> CargoResult<cargo::core::Package> {
        use cargo::core::{Source, SourceId, Registry};
        use cargo::core::Dependency as CargoDependency;
        use cargo::util::{Config, human};
        use cargo::sources::SourceConfigMap;

        // TODO: crates-license is only working for crates.io registry
        if !self.source.starts_with("registry") {
            Err(human("registry sources are unimplemented"))?;
        }

        let config = Config::default()?;
        let source_id = SourceId::from_url(&self.source)?;

        let source_map = SourceConfigMap::new(&config)?;
        let mut source = source_map.load(&source_id)?;

        // update crates.io-index registry
        source.update()?;

        let dep =
            CargoDependency::parse_no_deprecated(&self.name, Some(&self.version), &source_id)?;
        let deps = source.query(&dep)?;
        deps.iter()
            .map(|p| p.package_id())
            .max()
            .map(|pkgid| source.download(pkgid))
            .unwrap_or(Err(human("PKG download error")))
    }

    fn normalize(&self, license_string: &Option<String>) -> Option<String> {
        match license_string {
            &None => None,
            &Some(ref license) => {
                let mut list: Vec<&str> = license.split('/').collect();
                for elem in list.iter_mut() {
                    *elem = elem.trim();
                }
                list.sort();
                list.dedup();
                Some(list.join("/"))
            }
        }
    }

    pub fn get_authors(&self) -> CargoResult<Vec<String>> {
        let pkg = self.get_cargo_package()?;
        Ok(pkg.manifest().metadata().authors.clone())
    }

    pub fn get_license(&self) -> Option<String> {
        match self.get_cargo_package() {
            Ok(pkg) => {
                self.normalize(&pkg.manifest().metadata().license)
            }
            Err(_) => None,
        }
    }
}



pub fn get_dependencies_from_cargo_lock() -> Result<Vec<Dependency>> {
    let toml = {
        use std::fs::File;
        use std::io::Read;

        let lock_file = File::open("Cargo.lock")?;
        let mut reader = io::BufReader::new(lock_file);
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        content
    };

    // This code once was beautiful, but it became ugly after rustfmt
    let dependencies: Vec<Dependency> = toml::Parser::new(&toml)
                                                 .parse()
                                                 .as_ref()
                                                 .and_then(|p| p.get("package"))
                                                 .and_then(|p| p.as_slice())
                                                 .ok_or("Package not found")
                                                 .map(|p| {
        p.iter()
            .map(|p| {
                Dependency {
                    name: p.as_table()
                        .and_then(|n| n.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap()
                        .to_owned(),
                    version: p.as_table()
                        .and_then(|n| n.get("version"))
                        .and_then(|n| n.as_str())
                        .unwrap()
                        .to_owned(),
                    source: p.as_table()
                        .and_then(|n| n.get("source"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_owned(),
                }
            })
            .collect()
    })?;

    Ok(dependencies)
}



#[cfg(test)]
mod test {
    use super::get_dependencies_from_cargo_lock;

    #[test]
    fn test() {

        for dependency in get_dependencies_from_cargo_lock().unwrap() {
            assert!(!dependency.get_license().unwrap().is_empty());
        }
    }
}
