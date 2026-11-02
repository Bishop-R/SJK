use crate::InfoString;

/// Compatibility behavior selected from a server's advertised information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerDialect {
    BaseJka,
    /// LugorDev Lugormod-v3, identified by its advertised game name.
    LugormodV3,
    JaPlus {
        capabilities: JaPlusCapabilities,
    },
    /// TaystJK/jaPRO capabilities advertised in `jcinfo` and `taystJKinfo`.
    TaystJk {
        ja_pro_capabilities: JaProCapabilities,
        tayst_capabilities: TaystJkCapabilities,
    },
    Unknown {
        game_name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerProfile {
    pub dialect: ServerDialect,
    pub protocol: Option<i32>,
    pub max_clients: Option<u16>,
    pub server_fps: u16,
}

impl ServerProfile {
    pub fn from_server_info(info: &InfoString) -> Self {
        let protocol = info.get_i32("protocol");
        // `CS_SERVERINFO` commonly carries `gamename`; connectionless
        // `getinfo` replies commonly advertise the filesystem mod as `game`.
        let game_name = info
            .get("gamename")
            .or_else(|| info.get("game"))
            .unwrap_or("basejka");
        let lower_name = game_name.to_ascii_lowercase();

        let is_ja_plus = is_ja_plus_game_name(game_name);

        let is_tayst = lower_name.starts_with("japro") || lower_name.starts_with("taystjk");
        let dialect = if protocol.is_some_and(|version| version < 26) {
            // TaystJK deliberately treats the 1.00 protocol as BaseJKA even if
            // a mod name is present; preserve that compatibility behavior.
            ServerDialect::BaseJka
        } else if is_ja_plus {
            ServerDialect::JaPlus {
                capabilities: JaPlusCapabilities(
                    info.get_i32("jp_cinfo").unwrap_or_default() as u32
                ),
            }
        } else if is_tayst {
            ServerDialect::TaystJk {
                ja_pro_capabilities: JaProCapabilities(
                    info.get_i32("jcinfo").unwrap_or_default() as u32
                ),
                tayst_capabilities: TaystJkCapabilities(
                    info.get_i32("taystJKinfo").unwrap_or_default() as u32,
                ),
            }
        } else if is_lugormod_v3_game_name(game_name) && protocol.is_none_or(|p| p == 26) {
            ServerDialect::LugormodV3
        } else if lower_name.starts_with("basejk") || game_name.is_empty() {
            ServerDialect::BaseJka
        } else {
            ServerDialect::Unknown {
                game_name: game_name.to_owned(),
            }
        };

        let max_clients = info
            .get_i32("sv_maxclients")
            .and_then(|value| u16::try_from(value).ok());
        let server_fps = info
            .get_i32("sv_fps")
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(20);

        Self {
            dialect,
            protocol,
            max_clients,
            server_fps,
        }
    }
}

/// Advertised-name compatibility table. LugorDev/Lugormod-v3 50f42520
/// `game/g_local.h:64` publishes `^5L^7ugormod ^5v3`. Unversioned mod
/// directories and other Lugormod generations do not establish this policy.
const LUGORMOD_V3_NAMES: &[&[u8]] = &[b"lugormod v3"];

/// Match the documented v3 name table, ignoring legacy numeric colour escapes.
/// This allocation-free query can be used by presentation policy.
pub fn is_lugormod_v3_game_name(name: &str) -> bool {
    LUGORMOD_V3_NAMES.iter().any(|prefix| {
        let mut bytes = name.bytes().peekable();
        let mut next_plain = || loop {
            let byte = bytes.next()?;
            if byte == b'^' && bytes.peek().is_some_and(u8::is_ascii_digit) {
                bytes.next();
            } else {
                return Some(byte);
            }
        };
        prefix
            .iter()
            .all(|byte| next_plain().is_some_and(|ch| ch.eq_ignore_ascii_case(byte)))
            && next_plain().is_none_or(|ch| ch.is_ascii_whitespace() || ch == b'.')
    })
}

/// Whether a serverinfo `gamename` (or `getinfo` `game`) names JA+ or one of the
/// mods that present themselves as JA+. The aliases mirror names established JKA
/// clients recognise (EternalJK `cg_servercmds.c:247-250`). Allocation-free, so
/// presentation code can ask per frame.
pub fn is_ja_plus_game_name(game_name: &str) -> bool {
    let starts_with = |prefix: &str| {
        game_name
            .as_bytes()
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
    };
    starts_with("ja+")
        || starts_with("japlus")
        || starts_with("ja_plus")
        || game_name.starts_with("^4U^3A^5Galaxy")
        || starts_with("abyssmod")
}

/// jaPRO features encoded by the server in `jcinfo`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JaProCapabilities(pub u32);

impl JaProCapabilities {
    /// jaPRO `JAPRO_CINFO_FIXROLL1` (`bg_public.h:468`).
    pub const FIX_ROLL_1: u32 = 1 << 1;
    /// jaPRO `JAPRO_CINFO_FIXROLL2` (`bg_public.h:469`).
    pub const FIX_ROLL_2: u32 = 1 << 2;
    /// jaPRO `JAPRO_CINFO_FIXROLL3` (`bg_public.h:470`).
    pub const FIX_ROLL_3: u32 = 1 << 3;

    /// Whether all bits in `flag` are advertised.
    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

/// TaystJK-native features encoded by the server in `taystJKinfo`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TaystJkCapabilities(pub u32);

impl TaystJkCapabilities {
    /// TaystJK `TAYSTJK_INFO_FIXROLL_1` (`bg_public.h:554`).
    pub const FIX_ROLL_1: u32 = 1 << 4;
    /// TaystJK `TAYSTJK_INFO_FIXROLL_2` (`bg_public.h:555`).
    pub const FIX_ROLL_2: u32 = 1 << 5;
    /// TaystJK `TAYSTJK_INFO_FIXROLL_3` (`bg_public.h:556`).
    pub const FIX_ROLL_3: u32 = 1 << 6;

    /// Whether all bits in `flag` are advertised.
    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

/// JA+ features encoded by the server in `jp_cinfo`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JaPlusCapabilities(pub u32);

impl JaPlusCapabilities {
    pub const FLIP_KICK: u32 = 1 << 0;
    /// JA+ `JAPLUS_CINFO_FIXROLL1`, sourced by TaystJK
    /// `codemp/cgame/cg_local.h:100` at commit 5802c999.
    pub const FIX_ROLL_1: u32 = 1 << 1;
    /// JA+ `JAPLUS_CINFO_FIXROLL2`, sourced by TaystJK
    /// `codemp/cgame/cg_local.h:101` at commit 5802c999.
    pub const FIX_ROLL_2: u32 = 1 << 2;
    /// JA+ `JAPLUS_CINFO_FIXROLL3`, sourced by TaystJK
    /// `codemp/cgame/cg_local.h:102` at commit 5802c999.
    pub const FIX_ROLL_3: u32 = 1 << 3;
    pub const YELLOW_DFA: u32 = 1 << 4;
    pub const HEAD_SLIDE: u32 = 1 << 5;
    pub const SINGLE_PLAYER_ATTACKS: u32 = 1 << 6;
    pub const NEW_DFA: u32 = 1 << 7;
    pub const MODEL_SCALE: u32 = 1 << 8;
    pub const DAMAGE_SPEED_SCALE: u32 = 1 << 9;
    pub const MACRO_SCAN_1: u32 = 1 << 10;
    pub const MACRO_SCAN_2: u32 = 1 << 11;
    pub const JK2_DFA: u32 = 1 << 12;
    pub const NO_KATA: u32 = 1 << 13;
    pub const NO_AUTO_REPLIER: u32 = 1 << 14;
    pub const GLA_ANIMATIONS: u32 = 1 << 15;
    pub const LEDGE_GRAB: u32 = 1 << 16;
    pub const ALTERNATE_DIMENSION: u32 = 1 << 17;

    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lugormod_v3_name_table_requires_the_advertised_major_version() {
        for name in [
            "^5L^7ugormod ^5v3",
            "Lugormod v3",
            "lugormod v3.4",
            "LUGORMOD V3 release",
        ] {
            assert!(is_lugormod_v3_game_name(name), "{name}");
            let info = InfoString::parse(&format!("\\gamename\\{name}\\protocol\\26")).unwrap();
            assert_eq!(
                ServerProfile::from_server_info(&info).dialect,
                ServerDialect::LugormodV3
            );
        }
        for name in [
            "lugormod",
            "Lugormod U#",
            "Lugormod v2",
            "Lugormod v30",
            "japro",
            "basejka",
            "Lugormod v3extra",
        ] {
            assert!(!is_lugormod_v3_game_name(name), "{name}");
        }
        for protocol in [25, 27] {
            let info = InfoString::parse(&format!("\\gamename\\Lugormod v3\\protocol\\{protocol}"))
                .unwrap();
            assert_ne!(
                ServerProfile::from_server_info(&info).dialect,
                ServerDialect::LugormodV3
            );
        }
    }

    #[test]
    fn ja_plus_names_and_their_aliases_are_recognised() {
        for name in [
            "JA+ Mod v2.4 B7",
            "japlus",
            "JAPlus",
            "ja_plus",
            "^4U^3A^5Galaxy",
            "AbyssMod",
        ] {
            assert!(is_ja_plus_game_name(name), "{name}");
        }
        for name in ["basejka", "japro", "", "ja", "taystjk", "^4u^3a^5galaxy"] {
            assert!(!is_ja_plus_game_name(name), "{name}");
        }
    }
}
