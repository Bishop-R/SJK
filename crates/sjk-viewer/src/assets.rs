//! Retail-compatible viewer asset mounting and map loading.

use sjk_bsp::Bsp;
use sjk_entity::parse_entity_lump;
use sjk_shader::ShaderCatalog;
use sjk_vfs::VirtualFileSystem;
use std::error::Error;
use std::path::Path;

#[path = "download_store.rs"]
pub(crate) mod downloads;
#[path = "force_presets.rs"]
pub(crate) mod force_presets;
#[path = "generated_packs.rs"]
pub(crate) mod generated;
#[path = "asset_search_paths.rs"]
pub(crate) mod search_paths;
#[path = "session_content.rs"]
pub(crate) mod session_content;
#[path = "shader_authority.rs"]
mod shader_authority;

#[path = "world_input.rs"]
mod input;
#[path = "bsp_instances.rs"]
mod instances;
pub(crate) use input::GpuWorldInput;
pub(crate) use instances::prepare as prepare_instances;

pub(crate) fn load_bsp(
    game_data: &Path,
    map_path: &str,
) -> Result<(Bsp, VirtualFileSystem), Box<dyn Error>> {
    let vfs = mount_game_data(game_data)?;
    let bsp = load_bsp_from_vfs(&vfs, map_path)?;
    Ok((bsp, vfs))
}

/// Mount base PK3s and loose content without requiring a map.
pub(crate) fn mount_game_data(game_data: &Path) -> Result<VirtualFileSystem, Box<dyn Error>> {
    search_paths::startup().mount(game_data)
}

/// Optional map scripts must not prevent startup or joining unrelated maps.
pub(crate) fn load_shaders(vfs: &VirtualFileSystem) -> ShaderCatalog {
    shader_authority::load(vfs, search_paths::startup().protect_shaders)
}

pub(crate) fn load_shaders_until(
    vfs: &VirtualFileSystem,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Option<ShaderCatalog> {
    shader_authority::load_until(vfs, search_paths::startup().protect_shaders, cancelled)
}

pub(crate) fn load_bsp_from_vfs(
    vfs: &VirtualFileSystem,
    map_path: &str,
) -> Result<Bsp, Box<dyn Error>> {
    // A picture menu world has no map to read (`menu_pictures`).
    if crate::menu_pictures::pictured(map_path).is_some() {
        return Ok(crate::menu_pictures::empty_world());
    }
    let asset = vfs
        .read(map_path)?
        .ok_or_else(|| format!("map {map_path:?} was not found in the mounted game data"))?;
    let mut bsp = Bsp::parse(&asset.bytes)?;
    crate::external_lightmaps::load(vfs, map_path, &mut bsp);
    Ok(bsp)
}

pub(crate) fn initial_camera(bsp: &Bsp) -> Result<([f32; 3], f32), Box<dyn Error>> {
    let entities = parse_entity_lump(bsp.entities())?;
    for classname in [
        "info_player_deathmatch",
        "info_player_start",
        "info_player_duel",
        "info_player_intermission",
    ] {
        for entity in &entities {
            if entity.classname() != Some(classname) {
                continue;
            }
            if let Some(mut origin) = entity.vector("origin")? {
                origin[2] += 32.0;
                let yaw = entity.number("angle")?.unwrap_or(0.0).to_radians();
                crate::log::progress(format_args!(
                    "camera starts at {classname} origin {origin:?}"
                ));
                return Ok((origin, yaw));
            }
        }
    }
    let world = &bsp.render().models()[0];
    Ok((
        std::array::from_fn(|axis| (world.minimums[axis] + world.maximums[axis]) * 0.5),
        0.0,
    ))
}
