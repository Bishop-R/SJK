//! The interface between SJK and its client mods.
//!
//! A mod is a set of console commands and settings that live outside the core
//! client. Every name a mod owns starts with its id and a dot (`japlus.guntele`,
//! `japlus.loginServer1`), and its commands exist only on the servers it is made
//! for ([`ClientMod::loads_on`]) and while the player leaves it on (`mod_<id>`, on
//! by default). A mod reaches the game through [`Host`] alone: it sends server
//! commands and reads what the client already knows (the crosshair point, the
//! predicted player, the drawn players), and never touches the renderer, the
//! prediction or the wire directly.
//!
//! The mods are built into the client (see `docs/mods.md`); this crate is the line
//! they are written against.

pub mod known_servers;
mod player;
pub mod plugin_options;

pub use known_servers::{KNOWN_SERVERS, KnownServer};
pub use player::{Drawn, Player, resolve_player, strip_colours};

/// A console command a mod adds, or a server command it completes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command {
    /// The name as typed; a mod's own commands start with `<id>.`.
    pub name: &'static str,
    /// One line for the console's help and completion.
    pub help: &'static str,
}

/// Shorthand for a [`Command`] in a table.
pub const fn command(name: &'static str, help: &'static str) -> Command {
    Command { name, help }
}

/// A saved setting a mod owns; its name starts with `<id>.`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Setting {
    pub name: &'static str,
    pub default: SettingValue,
    pub help: &'static str,
}

/// Typed mod defaults, so switches and choices use the same settings widgets as core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingValue {
    /// A text field, including empty passwords and addresses.
    Text(&'static str),
    /// An archived on/off preference.
    Bool(bool),
    /// An integer choice.
    Integer(i64),
}

/// The game module a server runs, as far as mods care.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerKind {
    BaseJka,
    JaPlus,
    /// jaPRO and its TaystJK lineage.
    JaPro,
    Other,
}

/// The server the client is on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Server {
    pub kind: ServerKind,
    /// Its address, `host:port`.
    pub address: String,
    /// `CS_SERVERINFO`, a `\key\value` info string.
    pub info: String,
}

impl Server {
    /// The family this server belongs to, if SJK knows it ([`known_servers`]).
    pub fn family(&self) -> Option<&'static KnownServer> {
        known_servers::identify(self)
    }

    /// The value of `key` in the server info (keys ignore case).
    pub fn info(&self, key: &str) -> Option<&str> {
        let mut parts = self
            .info
            .strip_prefix('\\')
            .unwrap_or(&self.info)
            .split('\\');
        while let (Some(name), Some(value)) = (parts.next(), parts.next()) {
            if name.eq_ignore_ascii_case(key) {
                return Some(value);
            }
        }
        None
    }
}

/// The local player as the client predicts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Me {
    pub client: u16,
    pub origin: [f32; 3],
    /// Pitch, yaw, roll in degrees, as the game sends them.
    pub view_angles: [f32; 3],
    pub intermission: bool,
}

/// What the client lends a mod while one of its commands runs.
pub trait Host {
    /// Send a reliable command to the server.
    fn send(&mut self, command: &str) -> Result<(), String>;
    /// The server the client is on, if it is in a game.
    fn server(&self) -> Option<Server>;
    /// The local player, if it is in a game.
    fn me(&self) -> Option<Me>;
    /// Where the crosshair's trace ended this frame.
    fn crosshair_point(&self) -> Option<[f32; 3]>;
    /// The player under the crosshair (retained for a second, as the game does).
    fn crosshair_player(&self) -> Option<u16>;
    /// Every player in the roster, with where each is drawn when in view.
    fn players(&self) -> Vec<Player>;
    /// A setting's value as text.
    fn cvar(&self, name: &str) -> Option<String>;
    /// Change and save a setting.
    fn set_cvar(&mut self, name: &str, value: &str) -> Result<(), String>;
    /// Sweep the player's standing box from `start` to `end` through the map's
    /// world brushes, as movement would; `None` out of a game or without a map.
    fn trace_player(&self, start: [f32; 3], end: [f32; 3]) -> Option<Trace> {
        let _ = (start, end);
        None
    }
}

/// Where a box swept through the world stopped ([`Host::trace_player`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    /// How far along it got, 0 to 1 (1: nothing in the way).
    pub fraction: f32,
    /// Where the box stopped, clear of what it hit.
    pub end: [f32; 3],
    /// The box started inside something solid, so `end` is not to be trusted.
    pub start_solid: bool,
}

/// A validated field a loaded mod asks the existing userinfo adapter to send.
#[derive(Clone, Copy, Debug)]
pub struct UserinfoField {
    /// Server's existing userinfo key (not a new protocol field).
    pub key: &'static str,
    /// Nonempty opt-in value.
    pub value: &'static str,
    /// Archived mod setting that enables this opt-in.
    pub enabled_by: &'static str,
}

/// What a mod's servers take that must stay out of logs: passwords, which the
/// console masks in every line it prints or keeps (scrollback, `qconsole.log`, the
/// session log, crash reports and log uploads), and chat commands, whose typed lines
/// stay out of the session log, crash reports and log uploads as received chat does.
/// Every word is lowercase.
///
/// Unlike a mod's commands, a mod's secrets do not wait for its servers: the
/// client applies them from startup for every mod the build carries, whatever its
/// switch and the server, so a password typed offline or held in a bind is never
/// printed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Secrets {
    /// Commands whose arguments are a password (`amlogin`).
    pub commands: &'static [&'static str],
    /// Parts of a cvar name that make its value secret (`loginpass`).
    pub cvar_parts: &'static [&'static str],
    /// Commands whose arguments are a chat message (`amsay`).
    pub chat_commands: &'static [&'static str],
}

/// A client mod.
pub trait ClientMod: Send {
    /// Lower-case id: the prefix of its names and of its switch, `mod_<id>`.
    fn id(&self) -> &'static str;
    /// Its name in menus, such as "JA+ tools".
    fn title(&self) -> &'static str;
    /// One line on what it adds.
    fn about(&self) -> &'static str;
    /// The servers it loads on, for messages: "JA+" in "JA+ servers".
    fn servers(&self) -> &'static str;
    /// Whether it loads on `server`: its commands exist only there.
    fn loads_on(&self, server: &Server) -> bool;
    /// The commands it adds while loaded.
    fn commands(&self) -> &'static [Command];
    /// Its saved settings, kept whether it is on or not.
    fn settings(&self) -> &'static [Setting] {
        &[]
    }
    /// Earlier or other clients' names of its settings, as `(old, current)`: the
    /// host accepts them as aliases, so saved profiles and configs keep working
    /// and the value is saved under the current name.
    fn old_setting_names(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
    /// Userinfo opt-ins, applied only while this mod is loaded and the setting
    /// permits them. The host removes them on disable, leave or another server.
    fn userinfo_fields(&self) -> &'static [UserinfoField] {
        &[]
    }
    /// The passwords its servers take ([`Secrets`]), masked from startup.
    fn secrets(&self) -> Secrets {
        Secrets::default()
    }
    /// Server commands it completes on `server` while loaded (none by default).
    fn server_commands(&self, _server: &Server) -> &'static [Command] {
        &[]
    }
    /// Run one of [`ClientMod::commands`]; `name` is in lower case.
    fn run(
        &mut self,
        name: &str,
        args: &[String],
        host: &mut dyn Host,
    ) -> Result<Vec<String>, String>;
}

/// The unit vector of `angles` (pitch, yaw, roll in degrees), `AngleVectors`' forward.
pub fn forward(angles: [f32; 3]) -> [f32; 3] {
    let (pitch, yaw) = (angles[0].to_radians(), angles[1].to_radians());
    [
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        -pitch.sin(),
    ]
}

/// `atof`: the leading number of `text`, 0 when there is none.
pub fn number(text: &str) -> f32 {
    let text = text.trim_start();
    let end = text
        .char_indices()
        .take_while(|&(index, character)| {
            character.is_ascii_digit()
                || matches!(character, '.' | 'e' | 'E')
                || (index == 0 && matches!(character, '-' | '+'))
        })
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    // A trailing exponent marker or sign is not part of the number.
    (0..=end)
        .rev()
        .find_map(|end| text[..end].parse::<f32>().ok())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_server_info_keys_in_any_case() {
        let server = Server {
            kind: ServerKind::JaPlus,
            address: "127.0.0.1:29070".into(),
            info: r"\gamename\JA+ Mod v2.5\V\2.5B0\jp_cinfo\196819".into(),
        };
        assert_eq!(server.info("v"), Some("2.5B0"));
        assert_eq!(server.info("JP_CINFO"), Some("196819"));
        assert_eq!(server.info("mapname"), None);
    }

    #[test]
    fn forward_follows_angle_vectors() {
        let [x, y, z] = forward([0.0, 90.0, 0.0]);
        assert!(x.abs() < 1e-6 && (y - 1.0).abs() < 1e-6 && z.abs() < 1e-6);
        // Looking down (positive pitch) points below the horizon.
        assert!(forward([45.0, 0.0, 0.0])[2] < 0.0);
    }

    #[test]
    fn number_reads_like_atof() {
        assert_eq!(number("150"), 150.0);
        assert_eq!(number(" -32.5abc"), -32.5);
        assert_eq!(number("1e"), 1.0);
        assert_eq!(number("gun"), 0.0);
    }
}
