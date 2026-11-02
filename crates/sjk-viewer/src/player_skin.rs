//! Which skin a player appearance wears, chosen as the retail cgame chooses it
//! (`CG_RegisterClientModelname`, `codemp/cgame/cg_players.c`).
//!
//! A skin never makes a model fall back to Kyle there: when the requested skin
//! gives no skin handle (a file or part is missing, or it names no surface),
//! the cgame registers `model_default.skin` instead, and when that gives none
//! either the model draws the shaders its own surfaces name.
//!
//! An NPC's skin is named another way ([`SkinNaming::AppendedNpc`]): the skin the
//! server appended to its `CS_MODELS` string is three parts whenever it has a `|`,
//! and a part no file holds keeps the parts read before it, as the renderer's skin
//! cache hands them out.

use sjk_model::Skin;
use sjk_vfs::VirtualFileSystem;
use std::error::Error;

/// How the cgame turns a skin name into skin files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SkinNaming {
    /// A client's `model/skin` (`CG_RegisterClientModelname`, `cg_players.c:505-520`):
    /// three parts only when the name has `|`, `head`, `torso` and `lower`, and a
    /// missing part gives no skin.
    Client,
    /// The skin after the `*` of an NPC's model string (`CG_HandleAppendedSkin`,
    /// `cg_players.c:8519-8566`): three parts whenever it has a `|`. A part
    /// `RE_SplitSkins` names but no file holds, such as the empty lower part of
    /// `head_a3|torso_a3|` (the JoF Shuttle younglings' `customSkin`), leaves the
    /// parts read before it: `RE_RegisterSkin` returns 0 the first time but keeps
    /// the skin under that name with their surfaces, and every later registration
    /// of the name gets it (rd-vanilla `tr_skin.cpp:196-246`). SJK gives every NPC
    /// that settled skin rather than draw the first one of a kind without it.
    AppendedNpc,
}

/// The skin `directory` (`models/players/<name>`) wears for `variant`, named as
/// `naming` says.
pub(crate) fn resolve(
    vfs: &VirtualFileSystem,
    directory: &str,
    variant: &str,
    naming: SkinNaming,
) -> Result<Skin, Box<dyn Error>> {
    if let Some(skin) = requested(vfs, directory, variant, naming)? {
        return Ok(skin);
    }
    let fallback = read(vfs, &format!("{directory}/model_default.skin"))?
        .map(|bytes| Skin::parse(&bytes))
        .filter(|skin| !skin.is_empty());
    // `default` itself falls through to the surfaces' own shaders silently, as
    // machines such as the retail sentry ship no default skin.
    if variant != "default" {
        crate::log::progress(format_args!(
            "player skin {directory}/{variant} gives no skin; using {}",
            if fallback.is_some() {
                "model_default.skin"
            } else {
                "the model's own shaders"
            }
        ));
    }
    Ok(fallback.unwrap_or_default())
}

/// `RE_RegisterSkin` for the name the cgame builds; `None` stands for skin handle 0.
fn requested(
    vfs: &VirtualFileSystem,
    directory: &str,
    variant: &str,
    naming: SkinNaming,
) -> Result<Option<Skin>, Box<dyn Error>> {
    // A client's name is three parts only when it says which ones they are; an
    // NPC's whenever it has a `|`.
    let three_part = variant.contains('|')
        && (naming == SkinNaming::AppendedNpc
            || variant.contains("head") && variant.contains("torso") && variant.contains("lower"));
    if !three_part {
        let mut bytes = read(vfs, &format!("{directory}/model_{variant}.skin"))?;
        // BG_ValidateSkinForTeam (bg_misc.c:2687-2770) tries a custom team
        // suffix first, then the ordinary team skin if it is absent.
        if bytes.is_none() {
            let team = variant
                .strip_suffix("_red")
                .map(|_| "red")
                .or_else(|| variant.strip_suffix("_blue").map(|_| "blue"));
            if let Some(team) = team {
                bytes = read(vfs, &format!("{directory}/model_{team}.skin"))?;
            }
        }
        return Ok(bytes
            .map(|bytes| Skin::parse(&bytes))
            .filter(|skin| !skin.is_empty()));
    }
    // RE_SplitSkins: head and torso end at a `|`; the lower part is the rest.
    let parts = variant.splitn(3, '|').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Ok(None);
    }
    let mut skin = Skin::default();
    for (index, part) in parts.iter().enumerate() {
        // A part named like an earlier one is not read twice.
        if parts[..index].contains(part) {
            continue;
        }
        // RE_RegisterIndividualSkin returns 0 for a missing part, and for a
        // skin that still names no surface; the remaining parts are skipped.
        let Some(bytes) = read(vfs, &format!("{directory}/{part}.skin"))? else {
            // An NPC keeps the parts read so far, as the skin cache does.
            let kept = naming == SkinNaming::AppendedNpc && !skin.is_empty();
            return Ok(kept.then_some(skin));
        };
        skin.append(&bytes);
        if skin.is_empty() {
            return Ok(None);
        }
    }
    Ok(Some(skin))
}

fn read(vfs: &VirtualFileSystem, path: &str) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    Ok(vfs.read(path)?.map(|asset| asset.bytes))
}

#[cfg(test)]
mod tests {
    use super::{SkinNaming, resolve};
    use sjk_vfs::VirtualFileSystem;

    const MODEL: &str = "models/players/custom";

    fn vfs(files: &[(&str, &str)]) -> VirtualFileSystem {
        let mut vfs = VirtualFileSystem::new();
        vfs.mount_memory(
            "test",
            files
                .iter()
                .map(|(name, text)| (format!("{MODEL}/{name}"), text.as_bytes().to_vec())),
        )
        .expect("mount");
        vfs
    }

    fn shader(vfs: &VirtualFileSystem, variant: &str, surface: &str) -> Option<String> {
        resolve(vfs, MODEL, variant, SkinNaming::Client)
            .expect("resolve")
            .shader(surface)
            .map(str::to_owned)
    }

    #[test]
    fn a_named_skin_is_used() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("model_blue.skin", "torso,blue"),
        ]);
        assert_eq!(shader(&vfs, "blue", "torso").as_deref(), Some("blue"));
    }

    #[test]
    fn a_missing_skin_falls_back_to_the_default_skin() {
        let vfs = vfs(&[("model_default.skin", "torso,plain")]);
        assert_eq!(shader(&vfs, "original", "torso").as_deref(), Some("plain"));
    }

    #[test]
    fn a_skin_naming_no_surface_falls_back_to_the_default_skin() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("model_empty.skin", "// nothing yet\n"),
        ]);
        assert_eq!(shader(&vfs, "empty", "torso").as_deref(), Some("plain"));
    }

    #[test]
    fn without_any_skin_the_model_draws_its_own_shaders() {
        let vfs = vfs(&[]);
        assert!(
            resolve(&vfs, MODEL, "rgb", SkinNaming::Client)
                .expect("resolve")
                .is_empty()
        );
        assert!(
            resolve(&vfs, MODEL, "default", SkinNaming::Client)
                .expect("resolve")
                .is_empty()
        );
    }

    #[test]
    fn a_missing_team_skin_uses_the_plain_team_skin() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("model_red.skin", "torso,red"),
        ]);
        assert_eq!(shader(&vfs, "custom_red", "torso").as_deref(), Some("red"));
    }

    #[test]
    fn three_parts_combine_with_the_head_first() {
        let vfs = vfs(&[
            ("head_b1.skin", "head,face\ntorso,from_head"),
            ("torso_a1.skin", "torso,body"),
            ("lower_a1.skin", "There's nothing here!"),
        ]);
        let skin =
            resolve(&vfs, MODEL, "head_b1|torso_a1|lower_a1", SkinNaming::Client).expect("resolve");
        assert_eq!(skin.shader("head"), Some("face"));
        assert_eq!(skin.shader("torso"), Some("from_head"));
    }

    #[test]
    fn a_missing_part_falls_back_to_the_default_skin() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("head_b1.skin", "head,face"),
            ("torso_g1.skin", "torso,body"),
        ]);
        assert_eq!(
            shader(&vfs, "head_b1|torso_g1|lower_c2", "torso").as_deref(),
            Some("plain")
        );
        // The head part's surfaces are not kept either.
        assert!(shader(&vfs, "head_b1|torso_g1|lower_c2", "head").is_none());
    }

    #[test]
    fn a_name_without_head_torso_and_lower_is_one_skin_file() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("a|b|c.skin", "torso,parts"),
            ("model_a|b|c.skin", "torso,whole"),
        ]);
        assert_eq!(shader(&vfs, "a|b|c", "torso").as_deref(), Some("whole"));
    }

    fn npc_shader(vfs: &VirtualFileSystem, variant: &str, surface: &str) -> Option<String> {
        resolve(vfs, MODEL, variant, SkinNaming::AppendedNpc)
            .expect("resolve")
            .shader(surface)
            .map(str::to_owned)
    }

    #[test]
    fn an_npc_skin_with_a_bar_is_three_parts() {
        let vfs = vfs(&[
            ("head_a3.skin", "head,face"),
            ("torso_a3.skin", "torso,robe"),
            ("lower_a1.skin", "hips,legs"),
        ]);
        assert_eq!(
            npc_shader(&vfs, "head_a3|torso_a3|lower_a1", "hips").as_deref(),
            Some("legs")
        );
        assert_eq!(
            npc_shader(&vfs, "a3|b3|c3", "torso").as_deref(),
            None,
            "parts no file holds give no skin"
        );
    }

    #[test]
    fn an_npc_skin_keeps_the_parts_read_before_a_missing_one() {
        // JoF Shuttle's younglings: `customSkin head_a3|torso_a3|` and no default skin.
        let vfs = vfs(&[
            ("head_a3.skin", "head,face"),
            ("torso_a3.skin", "torso,robe"),
        ]);
        assert_eq!(
            npc_shader(&vfs, "head_a3|torso_a3|", "head").as_deref(),
            Some("face")
        );
        assert_eq!(
            npc_shader(&vfs, "head_a3|torso_a3|", "torso").as_deref(),
            Some("robe")
        );
        // A client's name that does not say `lower` stays one skin file.
        assert!(
            resolve(&vfs, MODEL, "head_a3|torso_a3|", SkinNaming::Client)
                .expect("resolve")
                .is_empty()
        );
    }

    #[test]
    fn an_npc_skin_whose_first_part_is_missing_falls_back() {
        let vfs = vfs(&[
            ("model_default.skin", "torso,plain"),
            ("torso_a3.skin", "torso,robe"),
        ]);
        assert_eq!(
            npc_shader(&vfs, "head_x|torso_a3|", "torso").as_deref(),
            Some("plain")
        );
    }
}
