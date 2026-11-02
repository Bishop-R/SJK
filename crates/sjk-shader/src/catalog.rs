//! Strict tooling and fault-isolated client shader catalogue loading.

use sjk_vfs::{VirtualFileSystem, VirtualPath};

use crate::{ShaderCatalog, ShaderDefinition, ShaderError, normalize_name, parse::parse_script};

mod authority;
pub use authority::DefinitionSource;

impl ShaderCatalog {
    /// Loads `.shader` and higher-priority `.oshader` scripts, returning the first error.
    /// Useful for validation tools which must detect broken content.
    pub fn load(vfs: &VirtualFileSystem) -> Result<Self, ShaderError> {
        let mut catalog = Self::default();
        for path in shader_paths(vfs) {
            catalog.load_script(vfs, &path)?;
        }
        Ok(catalog)
    }

    /// Loads usable definitions, reporting each malformed shader independently.
    /// Unreadable files are reported once. Valid override scripts retain `.oshader` priority.
    pub fn load_with_warnings(
        vfs: &VirtualFileSystem,
        warning: impl FnMut(&VirtualPath, &ShaderError),
    ) -> Self {
        Self::load_with_warnings_until(vfs, || false, warning)
            .expect("an uncancelled shader load completes")
    }

    /// Loads usable definitions, stopping before the next script read when
    /// `cancelled` reports that the owner no longer needs the catalogue.
    pub fn load_with_warnings_until(
        vfs: &VirtualFileSystem,
        mut cancelled: impl FnMut() -> bool,
        mut warning: impl FnMut(&VirtualPath, &ShaderError),
    ) -> Option<Self> {
        let mut catalog = Self::default();
        for path in shader_paths(vfs) {
            if cancelled() {
                return None;
            }
            match vfs.read(path.as_str()) {
                Ok(Some(asset)) => {
                    let definitions =
                        crate::recovery::parse_recovering(&asset.bytes, &path, |error| {
                            warning(&path, &error)
                        });
                    for definition in definition_order(definitions, &path) {
                        catalog
                            .definitions
                            .insert(normalize_name(&definition.name), definition);
                    }
                }
                Ok(None) => warning(&path, &ShaderError::DisappearedAsset(path.clone())),
                Err(error) => warning(&path, &ShaderError::Vfs(error)),
            }
        }
        Some(catalog)
    }

    fn load_script(
        &mut self,
        vfs: &VirtualFileSystem,
        path: &VirtualPath,
    ) -> Result<(), ShaderError> {
        let asset = vfs
            .read(path.as_str())?
            .ok_or_else(|| ShaderError::DisappearedAsset(path.clone()))?;
        for definition in definition_order(parse_script(&asset.bytes, path)?, path) {
            self.definitions
                .insert(normalize_name(&definition.name), definition);
        }
        Ok(())
    }
}

/// Normal scripts retain SJK's last-wins order. JoF EJK #138 places override
/// files before normal scripts in its first-wins lookup; reverse the override
/// paths/definitions here to produce the same precedence in our last-wins map.
fn shader_paths(vfs: &VirtualFileSystem) -> impl Iterator<Item = VirtualPath> {
    let paths = vfs.paths();
    let normal = paths
        .iter()
        .filter(|path| script_kind(path) == Some(false))
        .cloned();
    let overrides = paths
        .iter()
        .rev()
        .filter(|path| script_kind(path) == Some(true))
        .cloned();
    normal.chain(overrides).collect::<Vec<_>>().into_iter()
}

fn script_kind(path: &VirtualPath) -> Option<bool> {
    let name = path.as_str();
    if !name.starts_with("shaders/") {
        None
    } else if name.ends_with(".oshader") {
        Some(true)
    } else if name.ends_with(".shader") {
        Some(false)
    } else {
        None
    }
}

/// A script's definitions in insertion order for our last-wins map. Only the first of
/// a name in one script is kept, as rd-vanilla's `FindShaderInShaderText` finds the
/// first in the text: `maps/oot.bsp`'s `shaders/oot.shader` names its sand floor twice,
/// the second an unfinished copy without its lightmap stage that drew the floor as a
/// dark see-through plane (Sol's hub reports, 27/10/2026). Override scripts are then
/// reversed, so their first definition is inserted last.
fn definition_order(
    definitions: Vec<ShaderDefinition>,
    path: &VirtualPath,
) -> impl Iterator<Item = ShaderDefinition> {
    let mut seen = std::collections::HashSet::new();
    let mut definitions: Vec<_> = definitions
        .into_iter()
        .filter(|definition| seen.insert(normalize_name(&definition.name)))
        .collect();
    if script_kind(path) == Some(true) {
        definitions.reverse();
    }
    definitions.into_iter()
}

#[cfg(test)]
#[path = "catalog_overrides_tests.rs"]
mod override_tests;
