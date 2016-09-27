

extern crate cargo;
extern crate toml;
#[macro_use]
extern crate error_chain;


use std::collections::HashSet;
use std::io;


// I thought this crate is a good example to learn error_chain
// but looks like no need of it in this crate
error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links {}

    foreign_links {
        io::Error, Io, "IO Error";
        toml::Error, Toml, "TOML Error";
    }

    errors {}
}


#[derive(Debug)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub source: String,
    pub dependencies: Vec<Dependency>,
    pub license: Option<String>,
}


impl Dependency {
    fn get_cargo_package(&self) -> cargo::util::CargoResult<cargo::core::Package> {
        use cargo::core::{Source, SourceId, Registry};
        use cargo::core::Dependency as CargoDependency;
        use cargo::util::{Config, human};
        use cargo::sources::RegistrySource;

        // TODO: crates-license is only working for crates.io registry
        if !self.source.starts_with("registry") {
            unimplemented!();
        }

        let config = try!(Config::default());
        let source_id = SourceId::from_url(&self.source);
        let mut source = RegistrySource::new(&source_id, &config);

        // update crates.io-index registry
        try!(source.update());

        let dep = try!(CargoDependency::parse(&self.name, Some(&self.version), &source_id));
        let deps = try!(source.query(&dep));
        deps.iter()
            .map(|p| p.package_id())
            .max()
            .map(|pkgid| source.download(pkgid))
            .unwrap_or(Err(human("PKG download error")))
    }

    pub fn get_license(&mut self) -> String {
        // FIXME: So many N/A's
        if let Some(ref l) = self.license {
            l.clone()
        } else {
            self.license = Some(if !self.source.starts_with("registry") {
                "N/A".to_owned()
            } else {
                match self.get_cargo_package() {
                    Ok(pkg) => pkg.manifest().metadata().license.clone().unwrap_or("N/A".to_owned()),
                    Err(_) => "N/A".to_owned(),
                }
            });
            self.license.iter().next().cloned().unwrap()
        }
    }
}



pub fn get_dependencies_from_cargo_lock() -> Result<Vec<Dependency>> {
    let toml = {
        use std::fs::File;
        use std::io::Read;

        let lock_file = try!(File::open("Cargo.lock"));
        let mut reader = io::BufReader::new(lock_file);
        let mut content = String::new();
        try!(reader.read_to_string(&mut content));
        content
    };

    // This code once was beautiful, but it became ugly after rustfmt
    let dependencies: Vec<Dependency> = try!(toml::Parser::new(&toml)
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
                                                                        .and_then(|n| {
                                                                            n.get("version")
                                                                        })
                                                                        .and_then(|n| n.as_str())
                                                                        .unwrap()
                                                                        .to_owned(),
                                                              source: p.as_table()
                                                                       .and_then(|n| {
                                                                           n.get("source")
                                                                       })
                                                                       .and_then(|n| n.as_str())
                                                                       .unwrap_or("")
                                                                       .to_owned(),
                                                              dependencies: vec![],
                                                              license: None,
                                                          }
                                                      })
                                                      .collect()
                                                 }));

    Ok(dependencies)
}

pub fn get_dependency_tree() -> Result<Dependency> {
    let toml = {
        use std::fs::File;
        use std::io::Read;

        let lock_file = try!(File::open("Cargo.lock"));
        let mut reader = io::BufReader::new(lock_file);
        let mut content = String::new();
        try!(reader.read_to_string(&mut content));
        toml::Parser::new(&content).parse().unwrap()
    };

    let root_infos = toml["root"].as_table().unwrap();
    Ok(Dependency {
        name: root_infos["name"].as_str().unwrap().into(),
        version: root_infos["version"].as_str().unwrap().into(),
        source: "".into(),
        dependencies: try!(get_dependencies(
            toml["package"].as_slice().unwrap(),
            root_infos["dependencies"].as_slice().unwrap()
        )),
        license: None,
    })
}

fn get_dependencies(packages: &[toml::Value], items: &[toml::Value]) -> Result<Vec<Dependency>> {
    items.iter().map(|i| {
        let infos_str = try!(i.as_str().ok_or_else(missing_key));
        let (name, version) = {
            let mut splited = infos_str.splitn(3, ' ');
            (try!(splited.next().ok_or_else(missing_key)),
             try!(splited.next().ok_or_else(missing_key)))
        };
        let crate_infos = packages.iter().filter(|p| {
            let p = p.as_table().unwrap();
            (p["name"].as_str().unwrap() == name &&
             p["version"].as_str().unwrap() == version)

        }).next().unwrap().as_table().unwrap();
        let raw_dependencies = {
            match crate_infos.get("dependencies") {
                Some(deps) => deps.as_slice().unwrap(),
                None => &[],
            }

        };
        Ok(Dependency {
            name: crate_infos["name"].as_str().unwrap().into(),
            version: crate_infos["version"].as_str().unwrap().into(),
            source: crate_infos["source"].as_str().unwrap().into(),
            dependencies: try!(get_dependencies(packages, raw_dependencies)),
            license: None,
        })
    }).collect()
}

fn missing_key() -> toml::Error {
    toml::Error::Custom("wrong type".into())
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Incompatibility {
    pub crate1: String,
    pub crate1_version: String,
    pub crate1_license: String,
    pub crate2: String,
    pub crate2_version: String,
    pub crate2_license: String,
}

pub fn verify_license_compatibility(crate_infos: &mut Dependency) -> HashSet<Incompatibility> {
    let self_license = crate_infos.get_license();
    let mut incompatibles = HashSet::new();
    for dep in &mut crate_infos.dependencies {
        let dep_license = dep.get_license();
        if !is_compatible(&self_license, &dep_license) {
            incompatibles.insert(Incompatibility {
                crate1: crate_infos.name.clone(),
                crate1_version: crate_infos.version.clone(),
                crate1_license: self_license.clone(),
                crate2: dep.name.clone(),
                crate2_version: dep.version.clone(),
                crate2_license: dep_license.clone(),
            });
        }
        incompatibles.extend(verify_license_compatibility(dep));
    }
    incompatibles
}

// From http://www.dwheeler.com/essays/floss-license-slide.html
fn is_compatible(l1: &str, l2: &str) -> bool {
    for sub_license1 in l1.split("/") {
        for sub_license2 in l2.split("/") {
            let sub_license1 = sub_license1.trim();
            let sub_license2 = sub_license2.trim();
            // MIT/Apache-2.0 is compatible with GPL/MIT
            if sub_license1 == sub_license2 {
                return true;
            }
        }
    }
    false
}


#[cfg(test)]
mod test {
    use super::get_dependencies_from_cargo_lock;

    #[test]
    fn test() {

        for dependency in get_dependencies_from_cargo_lock().unwrap() {
            assert!(!dependency.get_license().is_empty());
        }
    }
}
