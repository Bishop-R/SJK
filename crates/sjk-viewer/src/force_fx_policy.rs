//! JoF EJK #130/#132/#168 presentation contracts; no server-name detection.
use crate::console::ViewerConsole;
use sjk_protocol::GameState;

/// Archived local switch for animation-driven Rage/Heal/Quick Heal and Grip-victim FX.
pub(crate) const ANIMATION_CVAR: &str = "cg_forceAnimFX";
/// Archived Empower shape override: negative follows the advertised server mode,
/// [`OFF_MODE`] hides the glow.
pub(crate) const EMPOWER_CVAR: &str = "cg_empowerEffect";
/// `JAPRO_PLUGIN_NEWFORCEEFFECT`, codemp/game/bg_public.h, EJK #130.
const DISABLE_NEW_FORCE: u32 = 1 << 5;
/// `EF_EMPOWERED`, EJK #132; only trims the existing EF_BODYPUSH presentation.
const EMPOWERED: u32 = 1 << 18;

// One entry per advertised contract. EJK #132 reads this serverinfo field;
// aliases for another compatible extension belong here, with their evidence.
const EMPOWER_KEYS: &[&str] = &[
    "jp_empowerEffect", // EJK #132 CG_ParseServerinfo: 0 full, 1 arms, 2 hands.
];
struct Scope {
    mode: i32,
    points: u8,
}
// Indices refer to force_bones::PUSH_BONE_NAMES; tests pin the authored names.
const SCOPES: &[Scope] = &[
    Scope {
        mode: 0,
        points: 0xff,
    }, // #132 cg_pushBoneNames: the ordinary full-body blur.
    Scope {
        mode: 1,
        points: 0xcc,
    }, // #132 cg_empowerArmBoneNames: hands and both radius joints.
    Scope {
        mode: 2,
        points: 0x0c,
    }, // #132 cg_empowerHandBoneNames: both hands only.
];
/// SJK's local "off" choice (Dante's idea in #ideas): no Empower body blur at all.
/// Not part of the #132 server contract, so an advertised 3 falls back to full body.
const OFF_MODE: i64 = SCOPES[SCOPES.len() - 1].mode as i64 + 1;
/// Highest mode offered by the shared settings catalogue.
pub(crate) const MAX_SCOPE: i64 = OFF_MODE;

/// Options sampled once at actor-frame submission, without allocation or locking.
#[derive(Clone, Copy)]
pub(crate) struct Policy {
    pub(crate) animation: bool,
    pub(crate) combined: bool,
    empower_points: u8,
}
impl Policy {
    pub(crate) fn sample(console: Option<&ViewerConsole>, game: Option<&GameState>) -> Self {
        let advertised = game
            .and_then(|game| game.config_string(0))
            .and_then(|info| {
                let info = sjk_client::LegacyClientInfo::new(info);
                EMPOWER_KEYS.iter().find_map(|key| info.integer(key))
            })
            .unwrap_or(0);
        Self::values(
            console
                .and_then(|c| c.bool_cvar("cg_forceanimfx"))
                .unwrap_or(true),
            console
                .and_then(|c| c.integer_cvar("cp_plugindisable"))
                .unwrap_or(i64::from(sjk_client::PLUGIN_DISABLE_DEFAULT)) as u32,
            advertised,
            console
                .and_then(|c| c.integer_cvar("cg_empowereffect"))
                .unwrap_or(-1),
        )
    }
    fn values(animation: bool, plugin_mask: u32, advertised: i32, override_mode: i64) -> Self {
        let enabled = plugin_mask & DISABLE_NEW_FORCE == 0;
        let mode = if override_mode < 0 {
            i64::from(advertised)
        } else {
            override_mode
        };
        let empower_points = if override_mode == OFF_MODE {
            0
        } else {
            SCOPES
                .iter()
                .find(|scope| i64::from(scope.mode) == mode)
                .unwrap_or(&SCOPES[0])
                .points
        };
        Self {
            animation: animation && enabled,
            combined: enabled,
            empower_points,
        }
    }
    /// Ordinary Push retains all eight points; only an Empower marker selects a subset.
    pub(crate) fn body_points(self, flags: u32) -> u8 {
        if flags & EMPOWERED != 0 {
            self.empower_points
        } else {
            SCOPES[0].points
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plugin_bit_disables_new_effects_and_combined_colour_without_changing_other_bits() {
        let ordinary = Policy::values(true, 1536, 0, -1);
        assert!(ordinary.animation && ordinary.combined);
        let disabled = Policy::values(true, 1536 | DISABLE_NEW_FORCE, 0, -1);
        assert!(!disabled.animation && !disabled.combined);
        let local_off = Policy::values(false, 1536, 0, -1);
        assert!(!local_off.animation && local_off.combined);
    }
    #[test]
    fn empower_modes_pin_the_reference_bones_and_do_not_trim_ordinary_push() {
        for (mode, expected) in [
            (
                0,
                vec![
                    "cranium",
                    "lower_lumbar",
                    "rhand",
                    "lhand",
                    "ltibia",
                    "rtibia",
                    "lradius",
                    "rradius",
                ],
            ),
            (1, vec!["rhand", "lhand", "lradius", "rradius"]),
            (2, vec!["rhand", "lhand"]),
        ] {
            let policy = Policy::values(true, 1536, mode, -1);
            let mask = policy.body_points(EMPOWERED | (1 << 19));
            let names: Vec<_> = crate::actor_pose::PUSH_BONE_NAMES
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, name)| *name)
                .collect();
            assert_eq!(names, expected);
            assert_eq!(policy.body_points(1 << 19), 0xff);
        }
    }
    #[test]
    fn player_override_and_unknown_modes_have_documented_fallbacks() {
        assert_eq!(
            Policy::values(true, 1536, 1, 2).body_points(EMPOWERED),
            0x0c
        );
        assert_eq!(
            Policy::values(true, 1536, 2, 0).body_points(EMPOWERED),
            0xff
        );
        assert_eq!(
            Policy::values(true, 1536, 99, -1).body_points(EMPOWERED),
            0xff
        );
        assert_eq!(
            Policy::values(true, 1536, 2, 99).body_points(EMPOWERED),
            0xff
        );
    }
    #[test]
    fn local_off_hides_only_the_empower_glow_and_is_not_a_server_mode() {
        for advertised in [0, 1, 2] {
            let off = Policy::values(true, 1536, advertised, OFF_MODE);
            assert_eq!(off.body_points(EMPOWERED | (1 << 19)), 0);
            assert_eq!(off.body_points(1 << 19), 0xff);
            assert!(off.animation && off.combined);
        }
        assert_eq!(
            Policy::values(true, 1536, OFF_MODE as i32, -1).body_points(EMPOWERED),
            0xff
        );
        assert_eq!(MAX_SCOPE, 3);
    }
    #[test]
    fn server_contract_is_sampled_without_a_mod_name_or_player_override() {
        let mut game = GameState::empty_local(0);
        game.replace_config_string(0, b"\\g_gametype\\0\\jp_empowerEffect\\2".to_vec())
            .unwrap();
        assert_eq!(
            Policy::sample(None, Some(&game)).body_points(EMPOWERED),
            0x0c
        );
        assert_eq!(Policy::sample(None, None).body_points(EMPOWERED), 0xff);
    }
    #[test]
    #[ignore = "release microbenchmark; run explicitly with --release --ignored --nocapture"]
    fn release_force_policy_cost() {
        let mut game = GameState::empty_local(0);
        game.replace_config_string(0, b"\\g_gametype\\0\\jp_empowerEffect\\1".to_vec())
            .unwrap();
        let start = std::time::Instant::now();
        for _ in 0..20000 {
            let policy = Policy::sample(None, std::hint::black_box(Some(&game)));
            for flags in 0..128 {
                std::hint::black_box(policy.body_points(std::hint::black_box(if flags & 1 == 0 {
                    EMPOWERED
                } else {
                    1 << 19
                })));
            }
        }
        eprintln!(
            "Force policy: {:.3} us/frame for server sampling + 128 body masks",
            start.elapsed().as_secs_f64() * 1e6 / 20000.0
        );
    }
}
