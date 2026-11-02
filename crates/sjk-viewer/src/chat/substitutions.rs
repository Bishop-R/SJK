//! EJK `CG_Say_f` substitutions, expanded once at submission before colouring,
//! quoting and the existing chat byte budget. No received text is interpreted.

use sjk_client::{ChatDestination, chat_command_full, chat_unescape};
use sjk_game_jka::pmove::MovementState;
use sjk_protocol::PlayerState;
use sjk_shell::local_time::LocalTime;
use std::borrow::Cow;
use std::fmt::Write as _;

/// Display labels from JoF EJK `codemp/cgame/cg_consolecmds.c::CG_Say_f`.
/// Unlisted weapons use its Saber fallback; these are compatibility labels.
const WEAPON_LABELS: &[(u8, &str)] = &[
    (1, "Stun baton"),
    (2, "Melee"),
    (4, "Pistol"),
    (5, "E11"),
    (6, "Sniper"),
    (7, "Bowcaster"),
    (8, "Repeater"),
    (9, "Demp2"),
    (10, "Flechette"),
    (11, "Rocket"),
    (12, "Thermal"),
    (13, "Tripmine"),
    (14, "Detpack"),
    (15, "Concussion rifle"),
    (16, "Bryar"),
];

/// Values belonging to the viewed player at the moment of submission.
pub(super) struct Values {
    health: i32,
    armor: i32,
    force: u8,
    weapon: u8,
    ammo: i32,
    speed: f32,
    ping: i32,
}

impl Values {
    pub(super) fn new(player: &PlayerState, predicted: Option<&MovementState>, ping: i32) -> Self {
        let weapon = predicted.map_or_else(|| player.weapon(), |p| p.weapon);
        let velocity = predicted.map_or_else(|| player.velocity(), |p| p.velocity);
        let ammo_index =
            sjk_game_jka::weapon_data::legacy_weapon_data(weapon).map(|data| data.ammo_index);
        Self {
            health: predicted.map_or_else(|| player.health(), |p| p.health),
            armor: player.armor(),
            force: predicted.map_or_else(|| player.force_power(), |p| p.force_power),
            weapon,
            ammo: ammo_index.map_or(0, |index| {
                predicted.map_or(player.ammo[index] as i32, |p| p.ammo[index])
            }),
            // cg.currentSpeed uses XY, independently of XYZ speedometer options.
            speed: (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt(),
            ping,
        }
    }
}

/// Expand complete whitespace-delimited words, without recursively expanding text.
/// Unknown tokens and punctuation remain literal. Clock tokens also work offline.
pub(super) fn expand<'a>(text: &'a str, values: Option<&Values>, time: LocalTime) -> Cow<'a, str> {
    if !text.contains('%') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end_matches(char::is_whitespace);
        let key = word.to_ascii_uppercase();
        let before = out.len();
        match (key.as_str(), values) {
            ("%T%", _) => {
                let hour = match time.hour % 12 {
                    0 => 12,
                    hour => hour,
                };
                let _ = write!(
                    out,
                    "{hour}:{:02} {}",
                    time.minute,
                    if time.hour < 12 { "AM" } else { "PM" }
                );
            }
            ("%T2%", _) => {
                let _ = write!(out, "{:02}:{:02}", time.hour, time.minute);
            }
            ("%H%", Some(v)) => {
                let _ = write!(out, "{}", v.health);
            }
            ("%S%", Some(v)) => {
                let _ = write!(out, "{}", v.armor);
            }
            ("%F%", Some(v)) => {
                let _ = write!(out, "{}", v.force);
            }
            ("%W%", Some(v)) => out.push_str(
                WEAPON_LABELS
                    .iter()
                    .find(|(id, _)| *id == v.weapon)
                    .map_or("Saber", |(_, name)| name),
            ),
            ("%A%", Some(v)) => {
                let _ = write!(out, "{}", v.ammo);
            }
            ("%P%", Some(v)) => {
                let _ = write!(out, "{}", v.ping);
            }
            ("%U%", Some(v)) => {
                let _ = write!(out, "{:.0}", (v.speed + 0.5).floor());
            }
            ("%UKM%", Some(v)) => {
                let _ = write!(out, "{:.1}", v.speed * 0.05);
            }
            ("%UM%", Some(v)) => {
                let _ = write!(out, "{:.1}", v.speed * 0.031_068_55);
            }
            _ => out.push_str(word),
        }
        changed |= &out[before..] != word;
        out.push_str(&piece[word.len()..]);
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(text)
    }
}

/// Normalize only an explicit outgoing game-chat command. The composer already
/// escaped percent signs; undo those display escapes before expanding, then use
/// the normal safe serializer again. The send boundary divides the result.
pub(super) fn command(command: &str, values: Option<&Values>, time: LocalTime) -> String {
    let Ok(tokens) = sjk_shell::tokenize(command) else {
        return command.to_owned();
    };
    let Some(name) = tokens.first() else {
        return command.to_owned();
    };
    let (destination, start) = if name.eq_ignore_ascii_case("say") {
        (ChatDestination::Global, 1)
    } else if name.eq_ignore_ascii_case("say_team") {
        (ChatDestination::Team, 1)
    } else if name.eq_ignore_ascii_case("amsay") {
        (ChatDestination::Admin, 1)
    } else if name.eq_ignore_ascii_case("tell") && tokens.len() >= 3 {
        let Ok(slot) = tokens[1].parse::<u16>() else {
            return command.to_owned();
        };
        if slot >= 32 {
            return command.to_owned();
        }
        (ChatDestination::Player(slot), 2)
    } else {
        return command.to_owned();
    };
    let text = tokens[start..].join(" ");
    let text = chat_unescape(&text);
    chat_command_full(destination, &expand(&text, values, time))
        .unwrap_or_else(|| command.to_owned())
}

impl crate::GpuState {
    pub(super) fn chat_substitution_values(&self) -> Option<Values> {
        let session = self
            .resident
            .session
            .as_ref()
            .or(self.live_session.as_ref())?;
        Some(Values::new(
            &session.latest_snapshot().player,
            self.local_prediction.predicted_state().filter(|state| {
                self.resident.session.is_none()
                    && state.client_num == session.latest_snapshot().player.client_num()
            }),
            self.lagometer.snapshot_newest(0).ping.max(0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(hour: u8) -> LocalTime {
        LocalTime {
            year: 2026,
            month: 10,
            day: 16,
            weekday: 5,
            hour,
            minute: 7,
            second: 0,
        }
    }
    fn values() -> Values {
        Values {
            health: 73,
            armor: 42,
            force: 61,
            weapon: 15,
            ammo: -1,
            speed: 250.0,
            ping: 38,
        }
    }

    #[test]
    fn all_eleven_tokens_follow_ejk_labels_units_and_precision() {
        assert_eq!(
            expand(
                "%H% %S% %F% %W% %A% %P% %U% %UKM% %UM% %T% %T2%",
                Some(&values()),
                time(13)
            ),
            "73 42 61 Concussion rifle -1 38 250 12.5 7.8 1:07 PM 13:07"
        );
    }

    #[test]
    fn words_are_case_insensitive_and_unknown_or_embedded_tokens_stay_literal() {
        assert_eq!(
            expand(
                "%h%\t%uKm%  health=%H% %S%, %%F%% %unknown% 100%",
                Some(&values()),
                time(0)
            ),
            "73\t12.5  health=%H% %S%, %%F%% %unknown% 100%"
        );
        assert_eq!(expand("%T% %T2% %H%", None, time(0)), "12:07 AM 00:07 %H%");
        assert_eq!(expand("%T%", None, time(12)), "12:07 PM");
    }

    #[test]
    fn console_binds_composer_team_and_tells_use_the_same_safe_expansion() {
        for (input, expected) in [
            ("say health %H%", "say \"health 73\""),
            ("say_team \"%S% %F%\"", "say_team \"42 61\""),
            ("tell 7 \"%W% %A%\"", "tell 7 \"Concussion rifle -1\""),
        ] {
            assert_eq!(command(input, Some(&values()), time(0)), expected);
        }
        let composer =
            chat_command_full(ChatDestination::Player(7), "%H% \"; quit\" 100%").unwrap();
        let sent = command(&composer, Some(&values()), time(0));
        assert_eq!(sent, "tell 7 \"73 ''; quit'' 100°/.\"");
        assert_eq!(sjk_shell::tokenize(&sent).unwrap().len(), 3);
        assert_eq!(
            command("set name %H%", Some(&values()), time(0)),
            "set name %H%"
        );
    }

    #[test]
    fn expansion_is_retained_for_the_send_boundary_to_divide() {
        let input = format!("say \"{}\"", "%W% 雪 ".repeat(80));
        let sent = command(&input, Some(&values()), time(0));
        let tokens = sjk_shell::tokenize(&sent).unwrap();
        assert!(sjk_protocol::encode_legacy_text(&tokens[1]).len() > sjk_client::CHAT_INPUT_BYTES);
        assert!(tokens[1].starts_with("Concussion rifle 雪"));
        assert!(tokens[1].ends_with("Concussion rifle 雪"));
    }

    #[test]
    fn speed_is_horizontal_and_ammo_uses_the_current_weapons_shared_pool() {
        let player = PlayerState::default();
        let mut predicted = MovementState::from_player_state(&player);
        predicted.velocity = [150.0, 200.0, 900.0];
        predicted.weapon = 15;
        predicted.ammo[4] = 22;
        let v = Values::new(&player, Some(&predicted), 37);
        assert_eq!(expand("%U% %A%", Some(&v), time(0)), "250 22");
    }
}
