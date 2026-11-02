//! LugorDev/Lugormod-v3 50f42520 multiplayer movement policy.
//!
//! Default CMake build: `LMD_NEW_FORCEPOWERS`/`LMD_NEW_WEAPONS` enabled,
//! `LMD_NEW_JETPACK`/`LMD_NEW_JUMP` absent. Only ordinary protocol-26 snapshot
//! fields are consumed. `cg_noPredict` remains the player override.
use sjk_protocol::{GameState, PlayerState};

// `game/bg_public.h` STAT_EXTRA_FORCE_BITS and FP_LEVITATION; Pmove packs
// each power's third level bit into the stat at that power's ordinal.
const EXTRA_FORCE_BITS: usize = 11;
const LEVITATION_BIT: u32 = 1 << 1;
// Ordinary protocol-26 player netfield `userInt1`, a one-bit value.
const USER_INT1: usize = 125;
const PM_DEAD: u8 = 5;

/// The current server's Lugormod movement policy; defaults to stock behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LugormodRules {
    /// Apply the referenced v3 rules to local players.
    pub enabled: bool,
}

impl LugormodRules {
    /// Select rules on each serverinfo refresh, without retaining a previous dialect.
    pub fn from_game_state(game: &GameState) -> Self {
        let enabled = game.config_string(0).is_some_and(|info| {
            let protocol = sjk_protocol::info_value(info, b"protocol")
                .and_then(|p| std::str::from_utf8(p).ok())
                .and_then(|p| p.parse::<i32>().ok());
            protocol.is_none_or(|p| p == 26)
                && sjk_protocol::info_value(info, b"gamename")
                    .and_then(|name| std::str::from_utf8(name).ok())
                    .is_some_and(sjk_protocol::is_lugormod_v3_game_name)
        });
        Self { enabled }
    }

    /// The grapple's rope fields are vehicle-only on the stock wire. Extended
    /// Force levels and scaled Force rises use curves this adapter does not simulate.
    /// Authored corpse hulls with their top below their feet are also unsupported.
    /// Preserve authoritative positions for those states, retaining immediate look.
    pub fn needs_snapshot(self, player: &PlayerState) -> bool {
        self.enabled
            && (player.movement_type() == PM_GRAPPLING
                || player.stats[EXTRA_FORCE_BITS] & LEVITATION_BIT != 0
                || (player.model_scale_percent() != 0
                    && player.model_scale_percent() != 100
                    && player.force_powers_active() & LEVITATION_BIT != 0)
                || (player.movement_type() == PM_DEAD
                    && -8.0 * LugormodState::from_player(player).scale() < -24.0))
    }
}

/// `pmtype_t::PM_GRAPPLING`, `game/bg_public.h:378`; not stock `PM_FLOAT`.
const PM_GRAPPLING: u8 = 9;

/// Snapshot-owned Lugormod data. Merely reading these reused fields never
/// enables a policy: the current server's [`LugormodRules`] must enable it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LugormodState {
    /// `iModelScale`: percent, with zero denoting unit scale.
    pub model_scale_percent: i32,
    /// `userInt1`, sent by `g_active.c:2637` for the enhanced jetpack.
    pub admin_jetpack: bool,
}

impl LugormodState {
    /// Read existing ordinary snapshot fields; no wire schema changes.
    pub fn from_player(player: &PlayerState) -> Self {
        Self {
            model_scale_percent: player.model_scale_percent(),
            admin_jetpack: player.raw_field(USER_INT1).unwrap_or(0) != 0,
        }
    }

    /// `Pmove`'s scale; malformed negative values retain a valid hull.
    pub(crate) fn scale(self) -> f32 {
        if self.model_scale_percent > 0 {
            self.model_scale_percent as f32 / 100.0
        } else {
            1.0
        }
    }
}
