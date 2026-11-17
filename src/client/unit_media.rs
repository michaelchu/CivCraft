//! Civ III unit catalog and owning-INI search policies, above filesystem lookup.
use super::{
    asset_error::AssetResult,
    media::{Civ3MediaResolver, MediaClass, ResolvedMedia},
};
use std::path::{Path, PathBuf};
pub(super) struct UnitMedia<'a> {
    media: &'a Civ3MediaResolver,
}
impl<'a> UnitMedia<'a> {
    pub fn new(media: &'a Civ3MediaResolver) -> Self {
        Self { media }
    }
    pub fn catalog_ini(&self, edition: &str, folder: &str) -> AssetResult<PathBuf> {
        let class = match edition {
            "base" => MediaClass::Base,
            "ptw" => MediaClass::Ptw,
            "conquests" => MediaClass::Conquests,
            _ => return Err(format!("Unknown unit art edition '{edition}'").into()),
        };
        self.media
            .classes(&[
                MediaClass::Scenario,
                MediaClass::Packaged,
                MediaClass::Explicit,
                class,
            ])
            .resolve(&format!("Art/Units/{folder}/{folder}.ini"))
    }
    fn resolver(&self, owner: &Path) -> AssetResult<Civ3MediaResolver> {
        let parent = owner.parent().ok_or("Media owner has no parent")?;
        let mut roots = vec![parent.to_owned()];
        if let Some(folder) = parent.file_name() {
            roots.extend(self.media.roots().map(|p| p.join("Art/Units").join(folder)));
        }
        roots.extend(self.media.prefixed("Art").roots().map(Path::to_owned));
        roots.extend(self.media.roots().map(Path::to_owned));
        Ok(self.media.searching(roots))
    }
    pub fn resolve(&self, owner: &Path, name: &str) -> AssetResult<PathBuf> {
        self.resolver(owner)?
            .resolve(name)
            .map_err(|e| e.context(format!("Unit INI {}", owner.display())))
    }
    pub fn lookup(&self, owner: &Path, name: &str) -> AssetResult<Option<ResolvedMedia>> {
        self.resolver(owner)?
            .lookup(name)
            .map_err(|e| e.context(format!("Unit INI {}", owner.display())))
    }
}
#[cfg(feature = "desktop-audio")]
use std::{collections::HashMap, fs};
#[cfg(feature = "desktop-audio")]
pub(super) fn build_unit_audio_index(
    media: &Civ3MediaResolver,
) -> AssetResult<HashMap<String, PathBuf>> {
    fn visit(
        directory: &Path,
        files: &mut HashMap<String, PathBuf>,
        visited: &mut std::collections::HashSet<PathBuf>,
        media: &Civ3MediaResolver,
    ) -> AssetResult<()> {
        let directory = match Civ3MediaResolver::new(vec![directory.to_owned()]).lookup(".")? {
            Some(found) => found.path,
            None => return Ok(()),
        };
        if !visited.insert(directory.clone()) {
            return Ok(());
        }
        media.existing(&directory)?;
        let mut entries = fs::read_dir(&directory)
            .map_err(|e| {
                super::asset_error::AssetError::io("read unit sound directory", &directory, e)
            })?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                super::asset_error::AssetError::io("read unit sound entry", &directory, e)
            })?;
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(&path, files, visited, media)?
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
            {
                let path = media.existing(&path)?;
                files
                    .entry(
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .to_ascii_lowercase(),
                    )
                    .or_insert(path);
            }
        }
        Ok(())
    }
    let mut files = HashMap::new();
    let mut visited = std::collections::HashSet::new();
    for root in media.prefixed("Art/Units").roots() {
        visit(root, &mut files, &mut visited, media)?
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "civcraft-unit-policy-{label}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
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
        assert_eq!(
            UnitMedia::new(&resolver)
                .resolve(&owner, "RUN.FLC")
                .unwrap(),
            shared
        );
        assert_eq!(
            UnitMedia::new(&resolver)
                .lookup(&owner, "RUN.FLC")
                .unwrap()
                .unwrap()
                .class,
            MediaClass::Base
        );
        fs::write(owner.parent().unwrap().join("local.flc"), "local").unwrap();
        assert_eq!(
            UnitMedia::new(&resolver)
                .lookup(&owner, "local.flc")
                .unwrap()
                .unwrap()
                .class,
            MediaClass::Conquests
        );

        assert_eq!(
            UnitMedia::new(&resolver)
                .resolve(&owner, "..\\..\\..\\..\\Art\\Units\\Archer\\run.flc")
                .unwrap(),
            shared
        );
        fs::write(root.join("blocked"), "file").unwrap();
        assert!(resolver.lookup("blocked/child").is_err());
        assert!(resolver.lookup("/absolute").is_err());
        assert!(resolver.lookup("genuinely-missing").unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
