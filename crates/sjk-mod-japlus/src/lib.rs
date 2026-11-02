//! JA+ tools: the client conveniences of JA+ and of the EternalJK family of
//! clients made for it, as an SJK mod (`japlus.*`, `docs/mods.md`).
//!
//! What a JA+ server needs to be played correctly stays in the core client (its
//! movement rules, `cjp_client`, `cp_pluginDisable` and its default, saber colours and
//! cosmetics): this mod adds commands a player types, the JA+ family's passwords the
//! console masks ([`secrets`]), JoF EternalJK's anti-kiss ([`anti_kiss`]) and the JA+
//! plugin options Settings switches ([`plugin`]).

pub mod anti_kiss;
mod login;
mod options;
pub mod plugin;
mod secrets;
mod server_commands;
mod teleport;

use sjk_mod::{ClientMod, Command, Host, Secrets, ServerKind, Setting, command};

/// The mod's id and prefix.
pub const ID: &str = "japlus";

const COMMANDS: &[Command] = &[
    command(
        "japlus.guntele",
        "Teleport to your crosshair, or a distance straight ahead, short of walls: japlus.guntele [distance] [yaw offset] (JA+ admin)",
    ),
    command(
        "japlus.bring",
        "Bring a player in front of you: japlus.bring [id|name|gun] [distance] [yaw offset] (JA+ admin)",
    ),
    command(
        "japlus.goto",
        "Teleport in front of a player: japlus.goto [id|name|gun] [distance] [yaw offset] (JA+ admin)",
    ),
    command(
        "japlus.teleoffset",
        "Teleport by an offset from where you are: japlus.teleoffset x [y [z]] (JA+ admin)",
    ),
    command("japlus.mark", "Remember where you stand for japlus.recall"),
    command(
        "japlus.recall",
        "Return to the japlus.mark position (setviewpos; the server must allow it)",
    ),
    command(
        "japlus.autologin",
        "Log in as admin with the password saved for this server (japlus.loginServer1-3)",
    ),
    command(
        "japlus.serverconfig",
        "List the JA+ server's options (asked of jaPRO servers)",
    ),
    command(
        "japlus.plugin",
        "List the JA+ plugin options (cp_pluginDisable) or switch one: japlus.plugin [number]",
    ),
];

const SETTINGS: &[Setting] = &[
    Setting {
        name: "japlus.loginServer1",
        default: sjk_mod::SettingValue::Text(""),
        help: "Server address for japlus.autologin (port 29070 when left out)",
    },
    Setting {
        name: "japlus.loginPass1",
        default: sjk_mod::SettingValue::Text(""),
        help: "Admin password sent to japlus.loginServer1 (saved as plain text)",
    },
    Setting {
        name: "japlus.loginServer2",
        default: sjk_mod::SettingValue::Text(""),
        help: "Server address for japlus.autologin (port 29070 when left out)",
    },
    Setting {
        name: "japlus.loginPass2",
        default: sjk_mod::SettingValue::Text(""),
        help: "Admin password sent to japlus.loginServer2 (saved as plain text)",
    },
    Setting {
        name: "japlus.loginServer3",
        default: sjk_mod::SettingValue::Text(""),
        help: "Server address for japlus.autologin (port 29070 when left out)",
    },
    Setting {
        name: "japlus.loginPass3",
        default: sjk_mod::SettingValue::Text(""),
        help: "Admin password sent to japlus.loginServer3 (saved as plain text)",
    },
    Setting {
        name: anti_kiss::CVAR,
        default: sjk_mod::SettingValue::Bool(false),
        help: "Push when a JA+ player puts you in the amkiss victim animation",
    },
];

/// Settings renamed into `japlus.`, under the names configs may still use.
const OLD_SETTING_NAMES: &[(&str, &str)] = &[(anti_kiss::EJK_CVAR, anti_kiss::CVAR)];

/// The JA+ tools mod.
#[derive(Default)]
pub struct JaPlus {
    mark: Option<teleport::Mark>,
}

impl JaPlus {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The local player, for the commands that need one.
fn me(host: &dyn Host) -> Result<sjk_mod::Me, String> {
    host.me().ok_or_else(|| "not in a game".to_owned())
}

impl ClientMod for JaPlus {
    fn id(&self) -> &'static str {
        ID
    }

    fn title(&self) -> &'static str {
        "JA+ tools"
    }

    fn about(&self) -> &'static str {
        "Admin teleports, auto login, plugin options and command completion for JA+ servers"
    }

    fn servers(&self) -> &'static str {
        "JA+"
    }

    /// Every JA+ server, JoF's included.
    fn loads_on(&self, server: &sjk_mod::Server) -> bool {
        server.kind == ServerKind::JaPlus
    }

    fn commands(&self) -> &'static [Command] {
        COMMANDS
    }

    fn settings(&self) -> &'static [Setting] {
        SETTINGS
    }

    fn old_setting_names(&self) -> &'static [(&'static str, &'static str)] {
        OLD_SETTING_NAMES
    }

    /// `amlogin`, the JA+ family's account and clan passwords and
    /// `japlus.loginPass1-3` ([`secrets`]).
    fn secrets(&self) -> Secrets {
        secrets::SECRETS
    }

    fn server_commands(&self, server: &sjk_mod::Server) -> &'static [Command] {
        if server.kind == ServerKind::JaPlus {
            server_commands::JA_PLUS
        } else {
            &[]
        }
    }

    fn run(
        &mut self,
        name: &str,
        args: &[String],
        host: &mut dyn Host,
    ) -> Result<Vec<String>, String> {
        let command = match name {
            "japlus.guntele" => {
                teleport::gun_tele(args, me(host)?, host.crosshair_point(), |from, to| {
                    host.trace_player(from, to)
                })?
            }
            "japlus.bring" => teleport::bring(args, host)?,
            "japlus.goto" => {
                let (command, note) = teleport::go_to(args, host)?;
                host.send(&command)?;
                return Ok(note.into_iter().collect());
            }
            "japlus.teleoffset" => teleport::offset(args, me(host)?)?,
            "japlus.mark" => {
                let mark = teleport::Mark::of(me(host)?);
                self.mark = Some(mark);
                return Ok(vec![mark.describe()]);
            }
            "japlus.recall" => self.mark.ok_or("No mark set: japlus.mark first")?.recall(),
            "japlus.autologin" => return login::auto_login(host),
            "japlus.serverconfig" => return options::server_config(host),
            "japlus.plugin" => return plugin::OPTIONS.run(args, host),
            _ => return Err(format!("Unknown JA+ tools command {name}")),
        };
        host.send(&command)?;
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_carries_the_prefix() {
        let mod_ = JaPlus::new();
        for name in mod_
            .commands()
            .iter()
            .map(|command| command.name)
            .chain(mod_.settings().iter().map(|setting| setting.name))
        {
            assert!(name.starts_with("japlus."), "{name}");
        }
        for slot in 1..=login::SLOTS {
            let (server, password) = login::names(slot);
            assert!(SETTINGS.iter().any(|setting| setting.name == server));
            assert!(SETTINGS.iter().any(|setting| setting.name == password));
        }
        for &(old, current) in mod_.old_setting_names() {
            assert!(!old.starts_with("japlus."), "{old}");
            assert!(SETTINGS.iter().any(|setting| setting.name == current));
        }
    }
}
