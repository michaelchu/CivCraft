//! Shared installation, scenario and packaged media resolution.
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MediaClass {
    Scenario,
    Packaged,
    Conquests,
    Ptw,
    Base,
    Explicit,
}
#[derive(Clone, Debug)]
pub(super) struct ResolvedMedia {
    pub path: PathBuf,
    pub class: MediaClass,
    pub root: PathBuf,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetRoots {
    pub install: Option<PathBuf>,
    #[serde(default)]
    pub scenarios: Vec<PathBuf>,
    #[serde(default)]
    pub packaged: Vec<PathBuf>,
    #[serde(default)]
    pub standalone: bool,
}
impl AssetRoots {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let mut config: Self = toml::from_str(
            &fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        for root in config
            .install
            .iter_mut()
            .chain(config.scenarios.iter_mut())
            .chain(config.packaged.iter_mut())
        {
            if root.is_relative() {
                *root = base.join(&*root);
            }
        }
        Ok(config)
    }
    pub fn from_env() -> Result<Self, String> {
        let mut config = if let Some(path) = std::env::var_os("CIVCRAFT_MEDIA_CONFIG") {
            Self::from_file(Path::new(&path))?
        } else {
            Self::default()
        };
        if let Some(path) =
            std::env::var_os("CIVCRAFT_SOURCE_DIR").or_else(|| std::env::var_os("CIV3_HOME"))
        {
            config.install = Some(path.into());
        }
        if !config.standalone && config.install.is_none() {
            return Err("Set CIVCRAFT_SOURCE_DIR (or CIV3_HOME), or configure standalone packaged media with CIVCRAFT_MEDIA_CONFIG".into());
        }
        if config.standalone && config.packaged.is_empty() {
            return Err("Standalone media requires at least one packaged root".into());
        }
        Ok(config)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Civ3MediaResolver {
    roots: Vec<(PathBuf, MediaClass)>,
    boundaries: Vec<PathBuf>,
    origins: Vec<(PathBuf, MediaClass)>,
}
impl Civ3MediaResolver {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self::classified(
            roots
                .into_iter()
                .map(|p| (p, MediaClass::Explicit))
                .collect(),
        )
    }
    fn classified(roots: Vec<(PathBuf, MediaClass)>) -> Self {
        let roots: Vec<_> = roots
            .into_iter()
            .map(|(p, c)| (resolve_case_insensitive_path(&p, ".").unwrap_or(p), c))
            .collect();
        let boundaries = roots.iter().map(|(p, _)| p.clone()).collect();
        Self {
            origins: roots.clone(),
            roots,
            boundaries,
        }
    }
    pub fn installation(root: &Path) -> Self {
        Self::classified(vec![
            (root.join("Conquests"), MediaClass::Conquests),
            (root.join("civ3PTW"), MediaClass::Ptw),
            (root.to_owned(), MediaClass::Base),
        ])
    }
    pub fn configured() -> Result<Self, String> {
        Ok(Self::from_config(&AssetRoots::from_env()?))
    }
    pub fn from_config(config: &AssetRoots) -> Self {
        if config.standalone {
            return Self::classified(
                config
                    .packaged
                    .iter()
                    .map(|p| (p.clone(), MediaClass::Packaged))
                    .collect(),
            );
        }
        let mut roots: Vec<_> = config
            .scenarios
            .iter()
            .map(|p| (p.clone(), MediaClass::Scenario))
            .chain(
                config
                    .packaged
                    .iter()
                    .map(|p| (p.clone(), MediaClass::Packaged)),
            )
            .collect();
        if let Some(install) = &config.install {
            roots.extend(Self::installation(install).roots);
        }
        Self::classified(roots)
    }
    pub fn prefixed(&self, prefix: &str) -> Self {
        Self {
            roots: self
                .roots
                .iter()
                .map(|(p, c)| (p.join(prefix), *c))
                .collect(),
            boundaries: self.boundaries.clone(),
            origins: self.origins.clone(),
        }
    }
    pub fn with_override(&self, root: &Path) -> Self {
        let mut result = self.clone();
        result
            .roots
            .insert(0, (root.to_owned(), MediaClass::Explicit));
        result.boundaries.push(root.to_owned());
        result
            .origins
            .insert(0, (root.to_owned(), MediaClass::Explicit));
        result
    }
    pub fn base_policy(&self) -> Self {
        let mut result = self.clone();
        result
            .roots
            .retain(|(_, c)| !matches!(c, MediaClass::Conquests | MediaClass::Ptw));
        result
    }
    pub fn roots(&self) -> impl Iterator<Item = &Path> {
        self.roots.iter().map(|(p, _)| p.as_path())
    }
    #[cfg(feature = "desktop-audio")]
    pub fn catalog_roots(&self) -> impl Iterator<Item = &Path> {
        self.roots
            .iter()
            .filter(|(_, c)| matches!(c, MediaClass::Scenario | MediaClass::Packaged))
            .map(|(p, _)| p.as_path())
    }
    pub fn lookup(&self, name: &str) -> io::Result<Option<ResolvedMedia>> {
        for (root, class) in &self.roots {
            let Some(path) = resolve_path(root, name)? else {
                continue;
            };
            let path = fs::canonicalize(path)?;
            let mut contained = false;
            for boundary in &self.boundaries {
                match fs::canonicalize(boundary) {
                    Ok(boundary) => contained |= path.starts_with(boundary),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            if !contained {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Media '{name}' escapes configured roots"),
                ));
            }
            let (source_root, source_class) = self
                .origins
                .iter()
                .find_map(|(source, class)| {
                    let source = source.canonicalize().ok()?;
                    path.starts_with(&source).then_some((source, *class))
                })
                .unwrap_or_else(|| (root.clone(), *class));
            let resolved = ResolvedMedia {
                path,
                class: source_class,
                root: source_root,
            };
            if std::env::var_os("CIVCRAFT_MEDIA_TRACE").is_some() {
                eprintln!(
                    "Media {name:?}: {:?} root={} path={}",
                    resolved.class,
                    resolved.root.display(),
                    resolved.path.display()
                );
            }
            return Ok(Some(resolved));
        }
        Ok(None)
    }
    pub fn resolve(&self, name: &str) -> Result<PathBuf, String> {
        self.lookup(name)
            .map_err(|e| format!("Media '{name}': {e}"))?
            .map(|m| m.path)
            .ok_or_else(|| self.missing(name))
    }
    pub fn missing(&self, name: &str) -> String {
        format!(
            "Media '{name}' not found; searched: {}",
            self.roots()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
    pub fn edition(&self, edition: &str) -> Self {
        let mut result = self.clone();
        result.roots.retain(|(_, class)| {
            matches!(
                class,
                MediaClass::Scenario | MediaClass::Packaged | MediaClass::Explicit
            ) || matches!(
                (edition, class),
                ("base", MediaClass::Base)
                    | ("ptw", MediaClass::Ptw)
                    | ("conquests", MediaClass::Conquests)
            )
        });
        result
    }
    pub fn lookup_variants(&self, names: &[String]) -> io::Result<Option<ResolvedMedia>> {
        for entry in &self.roots {
            let resolver = Self {
                roots: vec![entry.clone()],
                boundaries: self.boundaries.clone(),
                origins: self.origins.clone(),
            };
            for name in names {
                if let Some(found) = resolver.lookup(name)? {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }
    pub fn existing(&self, path: &Path) -> Result<PathBuf, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if self
            .boundaries
            .iter()
            .any(|root| root.canonicalize().is_ok_and(|root| path.starts_with(root)))
        {
            Ok(path)
        } else {
            Err(format!(
                "{} is outside configured media roots",
                path.display()
            ))
        }
    }
    pub fn unit_reference(&self, owner: &Path, name: &str) -> Result<PathBuf, String> {
        self.unit_lookup(owner, name)
            .map_err(|e| format!("{}: {name}: {e}", owner.display()))?
            .map(|found| found.path)
            .ok_or_else(|| self.missing(name))
    }
    pub fn unit_lookup(&self, owner: &Path, name: &str) -> io::Result<Option<ResolvedMedia>> {
        let parent = owner.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Media owner has no parent")
        })?;
        let mut roots = vec![(parent.to_owned(), MediaClass::Explicit)];
        if let Some(folder) = parent.file_name() {
            roots.extend(
                self.roots
                    .iter()
                    .map(|(p, c)| (p.join("Art/Units").join(folder), *c)),
            );
        }
        roots.extend(self.prefixed("Art").roots);
        roots.extend(self.roots.clone());
        let resolver = Self {
            roots,
            boundaries: self.boundaries.clone(),
            origins: self.origins.clone(),
        };
        resolver.lookup(name)
    }
}

fn resolve_path(base: &Path, reference: &str) -> io::Result<Option<PathBuf>> {
    let normalized = reference.trim().trim_matches('"').replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Media paths must be relative",
        ));
    }
    let mut current = match fs::metadata(base) {
        Ok(_) => base.to_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let Some(name) = base.file_name().and_then(|s| s.to_str()) else {
                return Ok(None);
            };
            let parent = base
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let Some(path) = resolve_path(parent, name)? else {
                return Ok(None);
            };
            path
        }
        Err(e) => return Err(e),
    };
    for component in normalized.split('/') {
        match component {
            "" | "." => continue,
            ".." => {
                if !current.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Invalid parent reference",
                    ));
                }
                continue;
            }
            _ => {}
        }
        let exact = current.join(component);
        match fs::metadata(&exact) {
            Ok(_) => {
                current = exact;
                continue;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut found = None;
        for entry in entries {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(component)
            {
                if found.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Ambiguous media component '{component}'"),
                    ));
                }
                found = Some(entry.path());
            }
        }
        let Some(path) = found else { return Ok(None) };
        current = path;
    }
    Ok(Some(current))
}
pub(super) fn resolve_case_insensitive_path(base: &Path, reference: &str) -> Option<PathBuf> {
    resolve_path(base, reference).ok().flatten()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("civcraft-media-{label}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
    }
    #[test]
    fn config_paths_are_relative_to_the_config_file_and_schema_is_checked() {
        let root = fixture("config-file");
        let file = root.join("media.toml");
        fs::write(
            &file,
            "install='install'\nscenarios=['scenario']\npackaged=['package']\nstandalone=false\n",
        )
        .unwrap();
        let config = AssetRoots::from_file(&file).unwrap();
        assert_eq!(config.install, Some(root.join("install")));
        assert_eq!(config.scenarios, vec![root.join("scenario")]);
        fs::write(&file, "unknown=true").unwrap();
        assert!(AssetRoots::from_file(&file).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn configured_precedence_provenance_and_standalone_isolation() {
        let root = fixture("config");
        let install = root.join("install");
        let scenario = root.join("scenario");
        let packaged = root.join("package");
        for folder in [
            &scenario,
            &packaged,
            &install.join("Conquests"),
            &install.join("civ3PTW"),
            &install,
        ] {
            fs::create_dir_all(folder.join("Art")).unwrap();
            fs::write(folder.join("Art/test.pcx"), "fixture").unwrap();
        }
        let config = AssetRoots {
            install: Some(install.clone()),
            scenarios: vec![scenario.clone()],
            packaged: vec![packaged.clone()],
            standalone: false,
        };
        let resolver = Civ3MediaResolver::from_config(&config);
        for (class, folder) in [
            (MediaClass::Scenario, scenario.clone()),
            (MediaClass::Packaged, packaged.clone()),
            (MediaClass::Conquests, install.join("Conquests")),
            (MediaClass::Ptw, install.join("civ3PTW")),
            (MediaClass::Base, install.clone()),
        ] {
            let found = resolver.lookup("Art/test.pcx").unwrap().unwrap();
            assert_eq!(found.class, class);
            assert_eq!(found.root, folder);
            fs::remove_file(folder.join("Art/test.pcx")).unwrap();
        }
        fs::write(install.join("Art/installed-only.pcx"), "base").unwrap();
        fs::write(scenario.join("Art/scenario-only.pcx"), "scenario").unwrap();
        let standalone = Civ3MediaResolver::from_config(&AssetRoots {
            standalone: true,
            ..config
        });
        assert!(
            standalone
                .lookup("Art/installed-only.pcx")
                .unwrap()
                .is_none()
        );
        assert!(
            standalone
                .lookup("Art/scenario-only.pcx")
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unit_owner_shared_references_and_invalid_optional_requests() {
        let root = fixture("owner");
        let owner = root.join("Conquests/Art/Units/Archer/Archer.ini");
        fs::create_dir_all(owner.parent().unwrap()).unwrap();
        fs::write(&owner, "ini").unwrap();
        let shared = root.join("Art/Units/Archer/run.flc");
        fs::create_dir_all(shared.parent().unwrap()).unwrap();
        fs::write(&shared, "run").unwrap();
        let resolver = Civ3MediaResolver::installation(&root);
        assert_eq!(resolver.unit_reference(&owner, "RUN.FLC").unwrap(), shared);
        assert_eq!(
            resolver
                .unit_lookup(&owner, "RUN.FLC")
                .unwrap()
                .unwrap()
                .class,
            MediaClass::Base
        );
        fs::write(owner.parent().unwrap().join("local.flc"), "local").unwrap();
        assert_eq!(
            resolver
                .unit_lookup(&owner, "local.flc")
                .unwrap()
                .unwrap()
                .class,
            MediaClass::Conquests
        );

        assert_eq!(
            resolver
                .unit_reference(&owner, "..\\..\\..\\..\\Art\\Units\\Archer\\run.flc")
                .unwrap(),
            shared
        );
        fs::write(root.join("blocked"), "file").unwrap();
        assert!(resolver.lookup("blocked/child").is_err());
        assert!(resolver.lookup("/absolute").is_err());
        assert!(resolver.lookup("genuinely-missing").unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_cannot_escape_configured_roots() {
        let root = fixture("symlink");
        let outside = fixture("outside");
        fs::write(outside.join("secret"), "outside").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        assert!(
            Civ3MediaResolver::new(vec![root.clone()])
                .lookup("escape/secret")
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    fn edition_precedence_case_and_diagnostics() {
        let root = std::env::temp_dir().join(format!("civcraft-media-{}", std::process::id()));
        for edition in ["", "civ3PTW", "Conquests"] {
            let folder = root.join(edition).join("Art/TeRrain");
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("Fog.PCX"), edition).unwrap();
        }
        let resolver = Civ3MediaResolver::installation(&root);
        let name = "art\\terrain\\fog.pcx";
        assert_eq!(
            fs::read_to_string(resolver.resolve(name).unwrap()).unwrap(),
            "Conquests"
        );
        fs::remove_dir_all(root.join("Conquests")).unwrap();
        assert_eq!(
            fs::read_to_string(resolver.resolve(name).unwrap()).unwrap(),
            "civ3PTW"
        );
        fs::remove_dir_all(root.join("civ3PTW")).unwrap();
        assert_eq!(
            fs::read_to_string(resolver.resolve(name).unwrap()).unwrap(),
            ""
        );
        assert!(
            resolver
                .resolve("missing")
                .unwrap_err()
                .contains("searched:")
        );
        assert!(resolver.resolve("/etc/passwd").is_err());
        fs::write(root.join("../civcraft-media-outside"), "outside").unwrap();
        assert!(resolver.resolve("../civcraft-media-outside").is_err());
        fs::remove_file(root.join("../civcraft-media-outside")).unwrap();
        assert!(resolver.resolve("C:\\Windows\\file").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
