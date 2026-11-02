//! Chat presentation and addressing at the protocol-26 compatibility boundary.
//!
//! OpenJK `codemp/game/g_cmds.c:G_SayTo` appends the sender slot to `chat`
//! and `tchat`; it is metadata, not part of the quoted message. `Cmd_Tell_f`
//! accepts `tell <slot> <message>`. No transport or wire encoding lives here.

use crate::{GameState, ServerEvent, ServerEventKind};

const CS_PLAYERS: usize = 1_131;
// codemp/qcommon/q_shared.h:890, protocol-26's absolute player-slot limit.
const MAX_CLIENTS: usize = 32;
/// Conservative compose budget, in UTF-8 bytes, below codemp's MAX_SAY_TEXT.
pub const CHAT_INPUT_BYTES: usize = 149;
/// Bounded in-memory draft; submission divides it into server-safe messages.
pub const CHAT_DRAFT_CHARS: usize = 1_200;
/// Most server messages one submitted draft may occupy.
pub const MAX_CHAT_PIECES: usize = 4;

/// A draft would keep the reliable channel occupied for too long.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatDraftTooLong;

/// Stock reliable-command destination; private messages address a numeric slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatDestination {
    /// Everyone the server permits to receive global chat.
    Global,
    /// The sender's team, subject to the server's game-mode policy.
    Team,
    /// One currently connected player.
    Player(u16),
    /// The server's admins: JA+'s and jaPRO's `amsay` (JoF EJK `Cmd_Amsay_f`).
    Admin,
}

/// A roster observation, invalidated on a name change or observed slot reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatTarget {
    slot: u16,
    generation: u64,
}

impl ChatTarget {
    /// Server-side client slot; callers must validate with the current roster.
    pub const fn slot(self) -> u16 {
        self.slot
    }
}

#[derive(Default)]
struct Player {
    // Cache wire spelling so lossy legacy-name decoding occurs only on identity changes.
    source_name: Vec<u8>,
    raw_name: String,
    name: String,
    generation: u64,
}

/// Bounded, presentation-only roster for chat actions. Refreshing an unchanged
/// gamestate borrows configstrings and does not allocate.
pub struct ChatRoster {
    players: [Player; MAX_CLIENTS],
}

impl Default for ChatRoster {
    fn default() -> Self {
        Self {
            players: std::array::from_fn(|_| Player::default()),
        }
    }
}

impl ChatRoster {
    /// Observe compact `CS_PLAYERS` name keys, including TaystJK's trailing `\`.
    pub fn update(&mut self, game: &GameState) {
        for (slot, player) in self.players.iter_mut().enumerate() {
            let name = game
                .config_string(CS_PLAYERS + slot)
                .and_then(player_name)
                .unwrap_or(b"");
            if player.source_name != name {
                player.source_name.clear();
                player.source_name.extend_from_slice(name);
                player.generation = player.generation.wrapping_add(1);
                player.raw_name.clear();
                // Latin-1, not UTF-8: see `legacy_text::decode_legacy`.
                player
                    .raw_name
                    .push_str(&crate::legacy_text::decode_legacy(name));
                player.name = chat_name_key(&player.raw_name);
            }
        }
    }

    /// Resolve server-provided metadata. Unattributed messages stay unattributed;
    /// displayed names are never guessed into private-message destinations.
    pub fn target(&self, sender: Option<u16>) -> Option<ChatTarget> {
        let slot = sender?;
        let player = self.players.get(usize::from(slot))?;
        (!player.name.is_empty()).then_some(ChatTarget {
            slot,
            generation: player.generation,
        })
    }

    /// Current name with its colour escapes, for display only; identity and
    /// destinations keep using [`ChatRoster::name`].
    pub fn display_name(&self, target: ChatTarget) -> Option<&str> {
        let player = self.players.get(usize::from(target.slot))?;
        (player.generation == target.generation && !player.raw_name.is_empty())
            .then_some(player.raw_name.as_str())
    }

    /// Current plain name, only while this exact roster observation is valid.
    pub fn name(&self, target: ChatTarget) -> Option<&str> {
        let player = self.players.get(usize::from(target.slot))?;
        (player.generation == target.generation && !player.name.is_empty())
            .then_some(player.name.as_str())
    }

    /// Resolve a typed name to one connected player's slot; see [`crate::lookup_player`].
    pub fn lookup(&self, query: &str) -> crate::PlayerLookup {
        crate::lookup_player(
            self.players.iter().enumerate().map(|(slot, player)| {
                (slot as u16, player.name.as_str(), player.raw_name.as_str())
            }),
            query,
        )
    }
}

fn player_name(config: &[u8]) -> Option<&[u8]> {
    let info = crate::LegacyClientInfo::new(config);
    info.bytes("n").or_else(|| info.bytes("name"))
}

/// Plain modern-chat text: remove Quake colour escapes and the stock chat
/// separator (0x19), retaining Unicode and ordinary punctuation.
pub fn chat_plain_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' && chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
        } else {
            push_chat_char(&mut output, c);
        }
    }
    output
}

/// A player's name as the roster's key (friends, `tell <name>`, private
/// messages): plain text whose Windows-1252 bytes 0x80..=0x9F (decoded as C1
/// controls) become the typographic characters they stand for (`’`, `€`, `…`), so
/// the name matches what a player types, and with every remaining control (a
/// vertical tab in `{JoF}\vToxiee`) removed. The displayed name keeps them all.
pub fn chat_name_key(value: &str) -> String {
    chat_plain_text(value)
        .chars()
        .map(|c| match u8::try_from(u32::from(c)) {
            Ok(byte @ 0x80..=0x9f) => sjk_protocol::windows_1252_char(byte),
            _ => c,
        })
        .filter(|c| !c.is_control())
        .collect()
}

/// Display chat text: drop the stock 0x19 separators but keep `^n` colour
/// escapes, which the text renderer applies per glyph.
pub fn chat_display_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for c in value.chars() {
        push_chat_char(&mut output, c);
    }
    output
}

/// Line breaks and tabs become spaces and the 0x19 separator is dropped. Other
/// control characters stay: a byte in 0x80..=0x9F decodes to the C1 control of
/// that value and is a Windows-1252 symbol (`€`, `’`, `…`), and a C0 control in
/// a name draws `.`, as it does in retail and EternalJK.
fn push_chat_char(output: &mut String, c: char) {
    match c {
        '\n' | '\r' | '\t' => output.push(' '),
        '\u{19}' | '\0' => {}
        _ => output.push(c),
    }
}

/// Chars of `value` with `^n` colour escapes skipped, as `(byte index, char)`.
fn uncoloured(value: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut skip = false;
    value.char_indices().filter(move |(index, c)| {
        if skip {
            skip = false;
            return false;
        }
        if *c == '^'
            && value[index + 1..]
                .chars()
                .next()
                .is_some_and(|n| n.is_ascii_digit())
        {
            skip = true;
            return false;
        }
        true
    })
}

/// Byte offset just past a prefix equal to `prefix` once colour codes are
/// ignored on both sides, so a server that colours the prefix differently from
/// the configstring name still splits.
fn skip_prefix(text: &str, prefix: &str) -> Option<usize> {
    let mut wanted = uncoloured(prefix).map(|(_, c)| c);
    let mut cursor = 0;
    for (index, c) in uncoloured(text) {
        let Some(expected) = wanted.next() else {
            // Stop right after the last matched character: a colour code
            // between the prefix and the body belongs to the body.
            return Some(cursor);
        };
        if c != expected {
            return None;
        }
        cursor = index + c.len_utf8();
    }
    wanted.next().is_none().then_some(cursor)
}

/// Separate only a known stock name prefix. Unfamiliar mod formatting is

/// retained in full. The prefix does not establish sender identity.
pub fn chat_body<'a>(text: &'a str, name: &str) -> (&'a str, bool) {
    for (open, close, private) in [("", ": ", false), ("(", "): ", false), ("[", "]: ", true)] {
        if let Some(at) = skip_prefix(text, open)
            .and_then(|at| Some(at + skip_prefix(&text[at..], name)?))
            .and_then(|at| Some(at + skip_prefix(&text[at..], close)?))
        {
            return (&text[at..], private);
        }
    }
    (text, false)
}

/// What EternalJK sends for `%`, which the engine turns into `.` on the way
/// (`MSG_ReadString`): `°/.`, the degree sign going out as byte 0xB0.
const PERCENT_ESCAPE: &str = "\u{b0}/.";
/// What EternalJK sends for `"`, which would end the quoted command.
const QUOTE_ESCAPE: &str = "''";

/// Construct one quoted stock command. Controls become spaces; `%` and `"` are
/// escaped as EternalJK's chat sends them (`Message_Key`, `cl_keys.cpp:896-930`
/// at JoF EternalJK bd5e202), so EternalJK clients and SJK show them again
/// ([`chat_unescape`]). The byte budget counts the bytes the text leaves with
/// ([`sjk_protocol::encode_legacy_text`]: one per Latin-1 or Russian letter, UTF-8
/// otherwise), always ends at a character boundary and never splits an escape.
pub fn chat_command(destination: ChatDestination, text: &str) -> Option<String> {
    if matches!(destination, ChatDestination::Player(slot) if usize::from(slot) >= MAX_CLIENTS) {
        return None;
    }
    let mut payload = String::with_capacity(CHAT_INPUT_BYTES);
    let mut buffer = [0; 4];
    for c in text.chars() {
        let before = payload.len();
        payload.push_str(chat_piece(c, &mut buffer));
        if sent_bytes(&payload) > CHAT_INPUT_BYTES {
            payload.truncate(before);
            break;
        }
    }
    command_from_payload(destination, payload.trim())
}

/// Construct one safe quoted command without applying the stock per-message bound.
/// The viewer uses this while editing and applies [`chat_commands`] immediately
/// before a command enters the reliable channel.
pub fn chat_command_full(destination: ChatDestination, text: &str) -> Option<String> {
    if matches!(destination, ChatDestination::Player(slot) if usize::from(slot) >= MAX_CLIENTS) {
        return None;
    }
    command_from_payload(destination, &escaped_payload(text)?)
}

/// Divide a chat message at word boundaries into safe quoted commands. A single
/// word longer than the compatibility budget is divided only at character
/// boundaries; EternalJK escapes are never cut in half.
pub fn chat_commands(
    destination: ChatDestination,
    text: &str,
) -> Result<Vec<String>, ChatDraftTooLong> {
    if matches!(destination, ChatDestination::Player(slot) if usize::from(slot) >= MAX_CLIENTS) {
        return Ok(Vec::new());
    }
    let mut commands = Vec::new();
    let mut rest = text.trim();
    let mut colour = None;
    while !rest.is_empty() {
        if commands.len() == MAX_CHAT_PIECES {
            return Err(ChatDraftTooLong);
        }
        let carry = colour.map(|digit| format!("^{digit}")).unwrap_or_default();
        let mut end = rest.len();
        let mut last_space = None;
        let mut overflow = false;
        for (index, c) in rest.char_indices() {
            let next = index + c.len_utf8();
            let candidate = format!("{carry}{}", &rest[..next]);
            if escaped_payload(&candidate)
                .is_some_and(|payload| sent_bytes(&payload) > CHAT_INPUT_BYTES)
            {
                end = last_space.filter(|space| *space > 0).unwrap_or(index);
                overflow = true;
                break;
            }
            if c.is_whitespace() {
                last_space = Some(index);
            }
        }
        if end == 0 {
            end = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        // Do not strand the introducer of a Quake colour escape at a hard split.
        if overflow
            && rest[..end].ends_with('^')
            && rest[end..].starts_with(|c: char| c.is_ascii_digit())
            && end > 1
        {
            end -= 1;
        }
        let original_piece = rest[..end].trim();
        let piece = format!("{carry}{original_piece}");
        if let Some(payload) = escaped_payload(&piece)
            && let Some(command) = command_from_payload(destination, &payload)
        {
            commands.push(command);
        }
        colour = last_colour(original_piece, colour);
        rest = rest[end..].trim_start();
    }
    Ok(commands)
}

/// The last Quake colour selected by `text`, or the colour carried into it.
fn last_colour(text: &str, mut colour: Option<char>) -> Option<char> {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '^'
            && let Some(digit) = chars.next()
            && digit.is_ascii_digit()
        {
            colour = Some(digit);
        }
    }
    colour
}

fn escaped_payload(text: &str) -> Option<String> {
    let mut payload = String::with_capacity(text.len() + 8);
    let mut buffer = [0; 4];
    for c in text.chars() {
        payload.push_str(chat_piece(c, &mut buffer));
    }
    let payload = payload.trim();
    if payload.is_empty() {
        return None;
    }
    Some(payload.to_owned())
}

fn command_from_payload(destination: ChatDestination, payload: &str) -> Option<String> {
    if payload.is_empty() {
        return None;
    }
    Some(match destination {
        ChatDestination::Global => format!("say \"{payload}\""),
        ChatDestination::Team => format!("say_team \"{payload}\""),
        ChatDestination::Player(slot) => format!("tell {slot} \"{payload}\""),
        ChatDestination::Admin => format!("amsay \"{payload}\""),
    })
}

/// What `c` becomes in a chat command: its escape, a space for a control, or
/// itself.
fn chat_piece(c: char, buffer: &mut [u8; 4]) -> &str {
    match c {
        '%' => PERCENT_ESCAPE,
        '"' => QUOTE_ESCAPE,
        c if c.is_control() => " ",
        c => c.encode_utf8(buffer),
    }
}

/// Bytes an escaped chat payload leaves with.
fn sent_bytes(payload: &str) -> usize {
    sjk_protocol::encode_legacy_text(payload).len()
}

/// Whether a draft fits the [`CHAT_INPUT_BYTES`] budget once escaped and encoded
/// as it is sent, so the chat field stops where the sent message would be cut, as
/// EternalJK's field leaves room for its escapes (`cl_keys.cpp:645-650`). The whole
/// line counts, since it picks the encoding: Russian takes a byte a letter when the
/// line leaves in Windows-1251, two in UTF-8, and a kanji three.
pub fn chat_input_fits(text: &str) -> bool {
    escaped_payload(text).is_none_or(|payload| sent_bytes(&payload) <= CHAT_INPUT_BYTES)
}

/// Show EternalJK's chat escapes as what was typed: `°/.` as `%` and `''` as
/// `"`, scanning left to right as `CG_ChatBox_AddString` does
/// (`cg_draw.c:9897-9916` at JoF EternalJK bd5e202). Text without them is
/// returned borrowed.
pub fn chat_unescape(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains(PERCENT_ESCAPE) && !text.contains(QUOTE_ESCAPE) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix(PERCENT_ESCAPE) {
            output.push('%');
            rest = after;
        } else if let Some(after) = rest.strip_prefix(QUOTE_ESCAPE) {
            output.push('"');
            rest = after;
        } else {
            output.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    std::borrow::Cow::Owned(output)
}

/// The end of the game server's copy of an SJK chat message (`docs/hub-chat.md`,
/// "Said on the game server too", after JoF EternalJK): the hub's id for the message
/// in colour codes, which no client draws: `^9`, then `^` and the number of digits,
/// then `^` and each digit, so id 12 ends a line with `^9^2^1^2`. `None` for an id of
/// more than nine digits, whose copy goes without it.
pub fn sjk_copy_marker(id: u64) -> Option<String> {
    let digits = id.to_string();
    if digits.len() > 9 {
        return None;
    }
    let mut marker = format!("^9^{}", digits.len());
    for digit in digits.chars() {
        marker.push('^');
        marker.push(digit);
    }
    Some(marker)
}

/// The hub's id a chat line's text ends with ([`sjk_copy_marker`]): `Some` for the
/// server's copy of an SJK chat message, `None` for any other line.
pub fn sjk_copy_id(text: &str) -> Option<u64> {
    copy_id(text.trim_end().as_bytes())
}

fn copy_id(text: &[u8]) -> Option<u64> {
    (1..=9u8).find_map(|digits| {
        let start = text.len().checked_sub(4 + 2 * usize::from(digits))?;
        let end = &text[start..];
        if end[..3] != *b"^9^" || end[3] != b'0' + digits {
            return None;
        }
        end[4..].chunks(2).try_fold(0u64, |id, pair| {
            (pair[0] == b'^' && pair[1].is_ascii_digit())
                .then(|| id * 10 + u64::from(pair[1] - b'0'))
        })
    })
}

/// Whether a server `chat` command carries the server's copy of an SJK chat message
/// ([`sjk_copy_id`]): its beep is left to the SJK chat's own sound.
pub(crate) fn sjk_copy_command(command: &[u8]) -> bool {
    let mut parts = command.splitn(2, u8::is_ascii_whitespace);
    if parts.next() != Some(b"chat".as_slice()) {
        return false;
    }
    let Some(rest) = parts.next() else {
        return false;
    };
    let Some(text) = rest.trim_ascii_start().strip_prefix(b"\"") else {
        return false;
    };
    let end = text
        .iter()
        .position(|&byte| byte == b'"')
        .unwrap_or(text.len());
    copy_id(&text[..end]).is_some()
}

/// What starts the game server's copy of an SJK chat message (and each of its pieces), so
/// the server's players on any client read it as an SJK line: SJK in gold, then All
/// chat's green again. JoF tools say such copies on JoF servers (`sjk-mod-jof`,
/// `sjk_chat.rs`); the core only reads them.
pub const SJK_COPY_TAG: &str = "^3SJK^2 ";

/// Interpret the already-tokenized stock chat command (`G_SayTo`).
pub(crate) fn server_chat_event(arguments: &[Vec<u8>]) -> Option<ServerEvent> {
    let command = arguments.first()?.as_slice();
    let kind = match command {
        b"chat" | b"lchat" => ServerEventKind::Chat,
        b"tchat" | b"ltchat" => ServerEventKind::TeamChat,
        _ => return None,
    };
    let localized = matches!(command, b"lchat" | b"ltchat");
    let sender_index = if localized { 5 } else { 2 };
    let sender = arguments
        .get(sender_index)
        .and_then(|arg| std::str::from_utf8(arg).ok())
        .and_then(|arg| arg.parse::<u16>().ok())
        .filter(|slot| usize::from(*slot) < MAX_CLIENTS);
    let text = if localized {
        // CG_Chat_f: location-bearing chat uses separate name/location/colour/body args.
        let [name, location, color, body] = [1, 2, 3, 4].map(|i| {
            arguments.get(i).map(|arg| {
                // The body is a whole message: one Russian letter reads as one.
                if i == 4 {
                    crate::legacy_text::decode_legacy_message(arg)
                } else {
                    crate::legacy_text::decode_legacy(arg)
                }
            })
        });
        let location = location?;
        // Use the client's existing localization marker for CG_Chat_f's @key.
        let marker = if location.starts_with('@') { "@@@" } else { "" };
        let location = location.strip_prefix('@').unwrap_or(&location);
        format!("{}^7<{marker}{location}> ^{}{}", name?, color?, body?)
    } else {
        crate::legacy_text::decode_legacy(arguments.get(1)?).into_owned()
    };
    Some(ServerEvent {
        kind,
        // G_Say's SAY_TELL to a teammate with a location sends `lchat`, whose
        // argument 1 is the same marked name.
        is_private: matches!(command, b"chat" | b"lchat")
            && arguments
                .get(1)
                .is_some_and(|text| text.starts_with(b"\x19[")),
        text,
        sender,
    })
}

#[cfg(test)]
mod display_text_tests {
    use super::{chat_display_text, chat_plain_text};

    #[test]
    fn chat_keeps_windows_1252_symbols_and_name_controls() {
        // Bytes 0x92, 0x91, 0x80 and 0x85 from another client, decoded as Latin-1.
        let received = "^6a\u{92}\u{91}\u{80}\u{85}×";
        assert_eq!(chat_display_text(received), received);
        assert_eq!(chat_plain_text(received), "a\u{92}\u{91}\u{80}\u{85}×");
        // A vertical tab in a name stays (drawn as `.`); the 0x19 separator and
        // line breaks do not.
        assert_eq!(
            chat_display_text("{JoF}\u{b}Toxiee\u{19}: hi\nthere"),
            "{JoF}\u{b}Toxiee: hi there"
        );
    }

    #[test]
    fn roster_keys_have_no_control_characters() {
        use super::chat_name_key;
        // ’ € … sent as bytes 0x92 0x80 0x85 match the characters a player types.
        assert_eq!(chat_name_key("^1a\u{92}b\u{80}\u{85}"), "a’b€…");
        // A vertical tab is dropped; an undefined 0x81 too.
        assert_eq!(
            chat_name_key("{JoF}\u{b}Toxiee\u{b}{C}.ak\u{81}"),
            "{JoF}Toxiee{C}.ak"
        );
    }
}

#[cfg(test)]
mod escape_tests {
    use super::*;

    #[test]
    fn percent_and_quotes_go_out_as_eternaljk_sends_them() {
        assert_eq!(
            chat_command(ChatDestination::Global, "100% \"done\"").as_deref(),
            Some("say \"100\u{b0}/. ''done''\"")
        );
        assert_eq!(
            chat_command(ChatDestination::Team, "50%").as_deref(),
            Some("say_team \"50\u{b0}/.\"")
        );
        assert_eq!(
            chat_command(ChatDestination::Player(3), "a\tb").as_deref(),
            Some("tell 3 \"a b\"")
        );
        // The degree sign leaves as the single byte EternalJK sends.
        let command = chat_command(ChatDestination::Global, "%").unwrap();
        assert_eq!(
            &*crate::legacy_text::legacy_command(command.as_bytes()),
            b"say \"\xb0/.\""
        );
    }

    #[test]
    fn the_input_fits_what_is_sent() {
        let full = |text: &str| {
            assert!(chat_input_fits(text), "{text}");
            let command = chat_command(ChatDestination::Global, text).unwrap();
            let sent = crate::legacy_text::legacy_command(command.as_bytes());
            assert_eq!(sent.len(), "say \"\"".len() + CHAT_INPUT_BYTES, "{text}");
            assert!(!chat_input_fits(&format!("{text}a")), "{text}");
        };
        full(&"a".repeat(CHAT_INPUT_BYTES));
        // `é` and Russian leave a byte a letter, as EternalJK sends them.
        full(&format!("{}caf\u{e9}", "caf\u{e9} ".repeat(29)));
        full(&"\u{436}".repeat(CHAT_INPUT_BYTES));
        // `%` leaves as `°/.`, three bytes; a kanji as three bytes of UTF-8.
        assert!(chat_input_fits(&"%".repeat(CHAT_INPUT_BYTES / 3)));
        assert!(!chat_input_fits(&"%".repeat(CHAT_INPUT_BYTES / 3 + 1)));
        assert!(chat_input_fits(&"\u{65e5}".repeat(CHAT_INPUT_BYTES / 3)));
        assert!(!chat_input_fits(
            &"\u{65e5}".repeat(CHAT_INPUT_BYTES / 3 + 1)
        ));
    }

    /// Every Russian letter alone, then short messages, typed in SJK and read as
    /// another SJK client reads the server's line (`Name^7\x19: ^2msg\x19`), and
    /// what an EternalJK Russian client gets: the Windows-1251 byte, never UTF-8.
    #[test]
    fn a_single_russian_letter_is_sent_and_read_as_russian() {
        let letters = ('\u{410}'..='\u{44f}').chain(['\u{401}', '\u{451}']);
        for letter in letters {
            let text = letter.to_string();
            assert_eq!(round_trip(&text), text);
            let command = chat_command(ChatDestination::Global, &text).unwrap();
            let sent = crate::legacy_text::legacy_command(command.as_bytes());
            let byte = sjk_protocol::windows_1251_byte(letter).unwrap();
            assert_eq!(
                &*sent,
                [b"say \"".as_slice(), &[byte], b"\""].concat(),
                "{text}"
            );
            let line = [b"Foshin^7\x19: ^2".as_slice(), &[byte], b"\x19"].concat();
            assert_eq!(
                crate::legacy_text::decode_legacy(&line),
                format!("Foshin^7\x19: ^2{text}\x19"),
                "{text}"
            );
            let one = [byte];
            let body = crate::legacy_text::decode_legacy_message(&one);
            assert_eq!(body, text, "a localized chat body");
        }
        for text in ["ДА", "НЕ", "я!", "Ж)", "1 я", "ага", "Привет", "лол xD"] {
            assert_eq!(round_trip(text), text);
        }
    }

    #[test]
    fn a_lone_western_accent_still_reads_back() {
        for text in ["ÿ", "é", "à", "ÆØ", "ß", "merci à tous", "Größe"] {
            assert_eq!(round_trip(text), text);
        }
    }

    #[test]
    fn the_budget_never_splits_an_escape_or_a_character() {
        let text = format!("{}%", "a".repeat(CHAT_INPUT_BYTES - 2));
        let command = chat_command(ChatDestination::Global, &text).unwrap();
        assert!(!command.contains('\u{b0}'), "{command}");
        assert_eq!(command.len(), "say \"\"".len() + CHAT_INPUT_BYTES - 2);
        // Kanji are cut whole: 49 of them (147 bytes), not 49 and two thirds.
        let text = "\u{65e5}".repeat(60);
        let command = chat_command(ChatDestination::Global, &text).unwrap();
        assert_eq!(command, format!("say \"{}\"", "\u{65e5}".repeat(49)));
    }

    #[test]
    fn long_chat_becomes_complete_server_safe_messages() {
        let text = format!("{}100% \"done\" {}", "word ".repeat(50), "雪".repeat(50));
        let commands = chat_commands(ChatDestination::Player(7), &text).expect("four pieces");
        assert!(commands.len() >= 3);
        let mut recovered = Vec::new();
        for command in commands {
            let payload = command
                .strip_prefix("tell 7 \"")
                .and_then(|body| body.strip_suffix('"'))
                .expect("safe tell command");
            assert!(sent_bytes(payload) <= CHAT_INPUT_BYTES);
            recovered.push(chat_unescape(payload).into_owned());
        }
        assert_eq!(recovered.concat().replace(' ', ""), text.replace(' ', ""));
        assert!(recovered.concat().contains("100% \"done\""));
    }

    #[test]
    fn a_draft_is_at_most_four_messages() {
        assert_eq!(
            chat_commands(ChatDestination::Global, &"%".repeat(1_200)),
            Err(ChatDraftTooLong)
        );
        let commands = chat_commands(ChatDestination::Global, &"%".repeat(190))
            .expect("four messages are accepted");
        assert_eq!(commands.len(), MAX_CHAT_PIECES);
    }

    #[test]
    fn a_split_message_carries_its_last_colour() {
        let text = format!("^1{}", "red words ".repeat(40));
        let commands = chat_commands(ChatDestination::Global, &text).expect("four pieces");
        assert!(commands.len() > 1);
        assert!(
            commands
                .iter()
                .all(|command| command.starts_with("say \"^1"))
        );
        assert!(commands.iter().all(|command| {
            let payload = command
                .strip_prefix("say \"")
                .and_then(|body| body.strip_suffix('"'))
                .expect("safe say command");
            sent_bytes(payload) <= CHAT_INPUT_BYTES
        }));
    }

    /// What arrives back at a client when `text` is typed in SJK's chat: the
    /// command's bytes as the server reads them (`MSG_ReadString` turns every `%`
    /// into `.`), the quoted message taken out, and the client's decoding and
    /// unescaping.
    fn round_trip(text: &str) -> String {
        let command = chat_command(ChatDestination::Global, text).unwrap();
        let bytes: Vec<u8> = crate::legacy_text::legacy_command(command.as_bytes())
            .iter()
            .map(|&byte| if byte == b'%' { b'.' } else { byte })
            .collect();
        let quoted = &bytes[b"say \"".len()..bytes.len() - 1];
        assert!(!quoted.contains(&b'"'), "a quote would end the message");
        // A byte in 0x80..=0x9F decodes as the C1 character of that value, which
        // the renderer draws as its Windows-1252 glyph (`€`, `’`, ...).
        let line = [b"Name^7\x19: ^2".as_slice(), quoted, b"\x19"].concat();
        let line = crate::legacy_text::decode_legacy(&line).into_owned();
        let shown = line
            .strip_prefix("Name^7\x19: ^2")
            .and_then(|line| line.strip_suffix('\x19'))
            .expect("the line keeps its frame");
        chat_unescape(shown)
            .chars()
            .map(|c| match u8::try_from(u32::from(c)) {
                Ok(byte @ 0x80..=0x9f) => sjk_protocol::windows_1252_char(byte),
                _ => c,
            })
            .collect()
    }

    #[test]
    fn every_typed_character_comes_back() {
        // Printable ASCII, Latin-1 and the characters Windows-1252 adds.
        let mut typed: Vec<char> = (0x20_u8..0x7f).map(char::from).collect();
        typed.extend((0xa0_u32..=0xff).filter_map(char::from_u32));
        typed.extend("€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ".chars());
        for c in typed {
            let text = format!("a{c}b");
            assert_eq!(round_trip(&text), text, "{c:?} (U+{:04X})", u32::from(c));
        }
        let mixed = "100% \"x\" ^1red ;// \\ ~`@#$&*()[]{}<>|=+-_!?.,:é€ü";
        assert_eq!(round_trip(mixed), mixed);
    }

    #[test]
    fn escapes_show_as_what_was_typed() {
        assert_eq!(chat_unescape("100\u{b0}/. ''done''"), "100% \"done\"");
        assert_eq!(chat_unescape("^1a\u{b0}/.^7b"), "^1a%^7b");
        // Left to right, as EternalJK: three apostrophes are a quote and one.
        assert_eq!(chat_unescape("\u{27}\u{27}\u{27}"), "\"\u{27}");
        assert_eq!(chat_unescape("\u{b0}/"), "\u{b0}/");
        assert!(matches!(
            chat_unescape("plain"),
            std::borrow::Cow::Borrowed("plain")
        ));
        // What SJK sends shows as what was typed.
        let sent = chat_command(ChatDestination::Global, "it's 100% \"fine\"").unwrap();
        let body = sent
            .strip_prefix("say \"")
            .unwrap()
            .strip_suffix('"')
            .unwrap();
        assert_eq!(chat_unescape(body), "it's 100% \"fine\"");
    }
}

/// Whether a reliable command carries the stock private-message marker before its name.
/// `lchat` (G_Say's tell to a teammate with a location) qualifies, its argument 1
/// being the marked name; team commands and lookalike colours or bodies do not.
pub(crate) fn private_chat_command(command: &[u8]) -> bool {
    let mut parts = command.splitn(2, u8::is_ascii_whitespace);
    if !parts
        .next()
        .is_some_and(|command| matches!(command, b"chat" | b"lchat"))
    {
        return false;
    }
    let Some(rest) = parts.next() else {
        return false;
    };
    let rest = rest.trim_ascii_start();
    rest.strip_prefix(b"\"")
        .unwrap_or(rest)
        .starts_with(b"\x19[")
}

#[cfg(test)]
mod private_marker_tests {
    use super::*;
    #[test]
    fn private_marker_precedes_name_and_only_belongs_to_global_chat() {
        for command in [
            b"chat \"\x19[Fox^7]: ^6hello\" 2".as_slice(),
            b"chat \x19[Fox]: hi 2",
            // G_Say's SAY_TELL to a teammate in a team game, with a location.
            b"lchat \"\x19[Fox^7\x19]\x19: \" \"@Base\" \"6\" \"hello\" 2",
        ] {
            assert!(private_chat_command(command));
            assert!(
                server_chat_event(&crate::tokenize_command(command))
                    .unwrap()
                    .is_private
            );
        }
        for command in [
            b"chat \"Fox: [Other^7]: ^6fake\" 2".as_slice(),
            b"chat \"Fox: \x19[Other]: fake\" 2",
            b"tchat \"\x19[Fox]: hi\" 2",
            b"lchat Fox 0 7 hi 2",
        ] {
            assert!(!private_chat_command(command));
            assert!(
                !server_chat_event(&crate::tokenize_command(command))
                    .unwrap()
                    .is_private
            );
        }
    }
}

#[cfg(test)]
mod sjk_copy_tests {
    use super::*;

    #[test]
    fn an_id_ends_the_line_in_colour_codes_and_reads_back() {
        assert_eq!(sjk_copy_marker(12).as_deref(), Some("^9^2^1^2"));
        assert_eq!(sjk_copy_marker(7).as_deref(), Some("^9^1^7"));
        assert_eq!(sjk_copy_marker(1_000_000_000), None);
        for id in [1, 9, 10, 99, 12_345, 999_999_999] {
            let line = format!("Sol^7: ^2hello{}", sjk_copy_marker(id).unwrap());
            assert_eq!(sjk_copy_id(&line), Some(id), "{line}");
        }
        assert_eq!(sjk_copy_id("Sol^7: ^2hello^9^2^1^2\n"), Some(12));
    }

    #[test]
    fn a_line_without_the_end_is_no_copy() {
        for line in [
            "Sol^7: ^2hello",
            "Sol^7: ^2hello ^9",
            "Sol^7: ^2hello^9^3^1^2",
            "Sol^7: ^2hello^8^2^1^2",
            "Sol^7: ^2hello^9^2^1x",
            "^2^1^2",
            "",
        ] {
            assert_eq!(sjk_copy_id(line), None, "{line}");
        }
    }

    #[test]
    fn only_a_chat_command_with_the_end_is_a_copy() {
        assert!(sjk_copy_command(b"chat \"Sol^7: ^2hello^9^2^1^2\" 3"));
        assert!(sjk_copy_command(b"chat \"Sol^7: ^2hello^9^1^5\""));
        assert!(!sjk_copy_command(b"chat \"Sol^7: ^2hello\" 3"));
        assert!(!sjk_copy_command(b"tchat \"Sol^7: ^2hello^9^2^1^2\" 3"));
        assert!(!sjk_copy_command(b"print \"hello^9^2^1^2\""));
    }
}
