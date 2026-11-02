//! The sender card a player's name shows when the pointer rests on it in a chat
//! (`docs/hub-chat.md`, "Muting a player"): in the game's chat while the composer is
//! open, on the main page's dock and on the SJK chat page. It shows what the SJK hub and
//! the server say of them, as the Players page's card does (their picture, or their
//! initial, as [`crate::profile_card::avatar`] draws it; their name and the verified
//! tick, whether the hub knows them, their key and hub name, their medals' medallions,
//! where they are on this server and how they were matched), and Mute or Unmute, which
//! hides their chat, draws them as Kyle with the default saber and silences them on this
//! PC only. For a player the hub vouches for, a sender it does not vouch for also gets
//! Ban 24 h beside it ([`crate::chat_ban`]). In the game's chat, where a click pins the
//! card, a player on the server also gets Whisper at the head of the buttons
//! ([`Tokens::whisper`], `chat/card.rs`).
//!
//! The card is built when the name under the pointer changes and drawn every frame
//! without allocating; each place that shows it keeps it up while the pointer moves
//! from the name onto the card (its [`Tokens::card`] target).

use crate::menu::sjk::color;
use crate::menu_widgets::MenuCanvas;
use sjk_ui::{Color, DrawCommand, FontWeight, Rect, TextAlign};

/// Where a player is on the server the player is on.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Place {
    /// Not playing on a server, or not known.
    #[default]
    Unknown,
    /// On a server, but not matched to any of its slots.
    Away,
    /// In this slot, by the hub's claim on it under the name the game shows (or the
    /// server's own word that the line is theirs).
    Slot(u8),
    /// In this slot, matched by name alone (colour codes ignored).
    SlotByName(u8),
}

/// Who a card is about.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Person {
    /// The name as shown in the chat, colour codes kept.
    pub(crate) name: String,
    /// Their SJK key, when the hub knows them.
    pub(crate) key_id: Option<String>,
    /// Their hub name, when it is not the name shown.
    pub(crate) hub_name: Option<String>,
    pub(crate) verified: bool,
    /// Their tick, as the hub sent it.
    pub(crate) tick: sjk_identity::Tick,
    pub(crate) staff: bool,
    /// SJK's creator (staff too).
    pub(crate) creator: bool,
    /// Their trust (`ui_renderer::mark_texture`), none when verified or staff or unknown.
    pub(crate) trust: Option<sjk_identity::Trust>,
    /// Their name emoji, after the name; empty for none.
    pub(crate) emoji: String,
    pub(crate) medals: crate::medals::Medals,
    pub(crate) place: Place,
    /// Their picture's version, empty for none; `None` while not known (the place that
    /// shows the card asks, outside any lock: `player_identity::avatar_version`).
    pub(crate) avatar: Option<String>,
    /// Validated banner id from the same profile lookup as the avatar.
    pub(crate) banner: &'static str,
    /// A line on when they were about, from the place that shows the card ("Online
    /// now, in a match", "Seen 3 minutes ago"), or none.
    pub(crate) note: Option<String>,
    /// They take encrypted whispers here: the card offers their safety code
    /// (`e2e_whisper_card.rs`).
    pub(crate) safety: bool,
    /// Their last encrypted whisper to the player can be reported: the card offers
    /// Report (`player_report.rs`).
    pub(crate) report: bool,
    /// They can be asked to trade from here (`crate::player_trades`): the card offers
    /// Trade.
    pub(crate) trade: bool,
    /// They are on the server being played: the card offers blocking them for this map
    /// (`crate::map_blocks`), and for everyone to staff.
    pub(crate) map_block: Option<MapBlockRow>,
}

/// The card's row of blocks for this map: whether the player blocked them for the map,
/// and, for an SJK staff player, whether staff blocked them for everyone (`None` for
/// the others, who are not offered it).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MapBlockRow {
    pub(crate) own: bool,
    pub(crate) staff: Option<bool>,
}

impl Person {
    /// The card's status line: whether the hub knows them.
    fn status(&self) -> &'static str {
        use sjk_identity::Trust;
        match (
            self.key_id.is_some(),
            self.staff || self.creator,
            self.trust,
        ) {
            (true, true, _) if self.creator => "SJK player, SJK creator",
            (true, true, _) => "SJK player, SJK staff",
            (true, false, Some(Trust::New)) => "SJK player, new",
            (true, false, Some(Trust::Trusted)) => "SJK player, trusted",
            (true, false, Some(Trust::Banned)) => "SJK player, banned from the chat",
            (true, false, _) => "SJK player",
            (false, ..) => "Not known to the SJK hub",
        }
    }
}

/// A card to draw: who, whether they are muted on this PC (the button then
/// unmutes), and how its text measures.
#[derive(Clone, Copy)]
pub(crate) struct Card<'a> {
    pub(crate) person: &'a Person,
    pub(crate) muted: bool,
    /// How the card's text measures, to set the tick after the name; without it the
    /// name's width is estimated.
    pub(crate) measure: Option<&'a crate::sjk_chat_look::Measure<'a>>,
}

/// The card's pointer targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Tokens {
    /// The card itself, which keeps it up under the pointer.
    pub(crate) card: u16,
    /// Mute or Unmute.
    pub(crate) mute: u16,
    /// Ban 24 h, when offered ([`crate::chat_ban::offered`]).
    pub(crate) ban: u16,
    /// Safety code, when [`Person::safety`].
    pub(crate) safety: u16,
    /// Report, when [`Person::report`].
    pub(crate) report: u16,
    /// Whisper, at the head of the card's buttons: only where the card can open a
    /// whisper to them (the game's chat, for a player on the server), else `None`.
    pub(crate) whisper: Option<u16>,
    /// Trade, when [`Person::trade`].
    pub(crate) trade: u16,
    /// Block for this map and, for staff, block for everyone, when [`Person::map_block`].
    pub(crate) map_block: u16,
    pub(crate) staff_block: u16,
}

/// The card's width in 1080p pixels: compact, so it sits beside the chat without
/// covering the game (Sol, 19/10/2026).
const WIDTH: f32 = 272.0;
const PAD: f32 = 12.0;
/// Medallions a card shows at most.
const MEDALS: usize = 6;
/// The picture's radius, beside the name and the status line (1080p pixels).
const PICTURE_RADIUS: f32 = 17.0;
/// The picture, the name and the status line.
const HEAD: f32 = 36.0;
/// The hairline under the head, with its room above and below.
const RULE: f32 = 13.0;
/// One line of what is known of them.
const LINE: f32 = 17.0;
const MEDAL_ROW: f32 = 26.0;
const BUTTON: f32 = 28.0;
/// Between the buttons and the quiet line under them, and that line's own height.
const QUIET_GAP: f32 = 5.0;
const QUIET_ROW: f32 = 14.0;

/// The card's size for `person`, at the scale `u` (1 at 1080 lines); `whisper` when it
/// offers Whisper ([`Tokens::whisper`]).
pub(crate) fn size(person: &Person, whisper: bool, u: f32) -> [f32; 2] {
    let mut height = PAD + HEAD + RULE;
    if whisper {
        height += BUTTON + 6.0;
    }
    if person.map_block.is_some() {
        height += BUTTON + 6.0;
    }
    let lines = [
        person.hub_name.is_some(),
        person.place != Place::Unknown,
        person.note.is_some(),
    ];
    height += lines.into_iter().filter(|shown| *shown).count() as f32 * LINE;
    if !person.medals.is_empty() {
        height += MEDAL_ROW;
    }
    height += 6.0 + BUTTON + QUIET_GAP + QUIET_ROW + PAD;
    if person.safety || person.report {
        height += BUTTON + 6.0;
    }
    if person.trade && ban_offered(person).is_some() {
        // Trade takes Ban's place beside Block: Ban gets a row of its own.
        height += BUTTON + 6.0;
    }
    if person.key_id.is_none() {
        height += QUIET_ROW;
    }
    [WIDTH * u, height * u]
}

/// Where a card of `size` goes beside `anchor` (the name): to its right if it fits in
/// `viewport`, else to its left, level with its top, kept on screen.
pub(crate) fn beside(anchor: Rect, size: [f32; 2], viewport: [f32; 2], gap: f32) -> [f32; 2] {
    let right = anchor.right() + gap;
    let x = if right + size[0] <= viewport[0] {
        right
    } else {
        (anchor.x - gap - size[0]).max(0.0)
    };
    let y = anchor.y.min(viewport[1] - size[1]).max(0.0);
    [x, y]
}

/// Draw `person`'s card with its top-left at `origin`, at the scale `u`; `muted` says
/// whether they are muted on this PC (the button then unmutes). Returns the card's
/// rectangle. Its targets are added last, over whatever lies under it.
pub(crate) fn draw(
    canvas: &mut MenuCanvas,
    card: &Card<'_>,
    origin: [f32; 2],
    u: f32,
    tokens: Tokens,
) -> Rect {
    let Card {
        person,
        muted,
        measure,
    } = *card;
    let [width, height] = size(person, tokens.whisper.is_some(), u);
    let card = Rect::new(origin[0], origin[1], width, height);
    let list = canvas.draw_list_mut();
    // A soft shadow, the panel, its hairline edge and a thin accent along the top.
    let _ = list.push(DrawCommand::RoundedRect {
        rect: Rect::new(card.x + 2.0 * u, card.y + 4.0 * u, width, height),
        radius: 9.0 * u,
        color: Color::new(0.0, 0.0, 0.0, 0.35),
    });
    let _ = list.push(DrawCommand::RoundedRect {
        rect: card,
        radius: 9.0 * u,
        color: color::alpha(color::SPACE, 0.97),
    });
    let _ = list.push(DrawCommand::PushRoundedClip {
        rect: card,
        radius: 9.0 * u,
    });
    crate::profile_card::banner::draw(
        canvas,
        Rect::new(
            card.x + 1.0 * u,
            card.y + 1.0 * u,
            width - 2.0 * u,
            (HEAD + 2.0 * PAD) * u,
        ),
        person.banner,
    );
    let list = canvas.draw_list_mut();
    let _ = list.push(DrawCommand::PopClip);
    let _ = list.push(DrawCommand::Border {
        rect: card,
        radius: 9.0 * u,
        width: 1.0 * u,
        color: color::alpha(color::HOLO, 0.28),
    });
    let _ = list.push(DrawCommand::RoundedRect {
        rect: Rect::new(card.x + 14.0 * u, card.y, width - 28.0 * u, 2.0 * u),
        radius: 1.0 * u,
        color: color::alpha(
            if person.key_id.is_some() {
                color::GOLD
            } else {
                color::HOLO
            },
            0.7,
        ),
    });
    let x = card.x + PAD * u;
    let inner = width - 2.0 * PAD * u;
    let mut y = card.y + PAD * u;
    let line = |canvas: &mut MenuCanvas,
                y: f32,
                size: f32,
                colour: Color,
                value: std::fmt::Arguments<'_>| {
        canvas.text_fmt_aligned(
            value,
            Rect::new(x, y, inner, size * 1.35),
            size,
            colour,
            FontWeight::Regular,
            0.0,
            TextAlign::Start,
        );
    };
    // Their picture, or their initial until it is loaded or when they have none,
    // beside the name and the status line. The tick after the name says verified.
    let radius = PICTURE_RADIUS * u;
    crate::profile_card::avatar(
        canvas,
        [x + radius, y + HEAD * 0.5 * u],
        radius,
        &crate::profile_card::Avatar {
            key_id: person.key_id.as_deref().unwrap_or(""),
            version: person.avatar.as_deref().unwrap_or(""),
            name: &person.name,
            verified: false,
            tick: sjk_identity::Tick::None,
            trust: None,
            preview: false,
            lit: false,
        },
    );
    let head_x = x + radius * 2.0 + 10.0 * u;
    let head_width = (inner - (head_x - x)).max(1.0);
    // The name and its mark alone: the verified seal or the trust mark.
    let name_size = 16.0 * u;
    let mark = crate::ui_renderer::mark_texture(person.verified, person.tick, person.trust);
    let tick_room = if mark.is_some() {
        crate::sjk_chat_look::tick_room(name_size)
    } else {
        0.0
    };
    // The name emoji right after the name, before the mark.
    let shown = if person.emoji.is_empty() {
        person.name.clone()
    } else {
        format!("{}^7 {}", person.name, person.emoji)
    };
    canvas.text_fmt_aligned(
        format_args!("{shown}"),
        Rect::new(head_x, y, head_width - tick_room, 22.0 * u),
        name_size,
        color::TEXT,
        FontWeight::Semibold,
        0.0,
        TextAlign::Start,
    );
    if let Some(mark) = mark {
        let name = match measure {
            Some(measure) => measure.width(&shown, name_size, crate::text::TextFace::Semibold),
            None => estimated_width(&shown, name_size),
        };
        let end = head_x + (head_width - tick_room).min(name);
        crate::sjk_chat_look::tick(
            canvas.draw_list_mut(),
            mark,
            end,
            y + 10.5 * u,
            name_size,
            1.0,
        );
    }
    canvas.text_fmt_aligned(
        format_args!("{}", person.status()),
        Rect::new(head_x, y + 21.0 * u, head_width, 12.5 * u * 1.35),
        12.5 * u,
        if person.key_id.is_some() {
            color::GOLD_BRIGHT
        } else {
            color::MUTED
        },
        FontWeight::Regular,
        0.0,
        TextAlign::Start,
    );
    y += HEAD * u;
    let _ = canvas.draw_list_mut().push(DrawCommand::SolidRect {
        rect: Rect::new(x, y + 6.0 * u, inner, 1.0 * u),
        color: color::alpha(color::HOLO, 0.14),
    });
    y += RULE * u;
    let info = 12.0 * u;
    if let Some(hub) = &person.hub_name {
        line(
            canvas,
            y,
            info,
            color::MUTED,
            format_args!("Known as {hub}"),
        );
        y += LINE * u;
    }
    let (slot, how) = match person.place {
        Place::Unknown => (None, ""),
        Place::Away => (None, "Not on this server"),
        Place::Slot(slot) => (Some(slot), ""),
        Place::SlotByName(slot) => (Some(slot), " (by name)"),
    };
    match slot {
        Some(slot) => line(
            canvas,
            y,
            info,
            color::MUTED,
            format_args!("On this server, slot {slot}{how}"),
        ),
        None if !how.is_empty() => line(canvas, y, info, color::QUIET, format_args!("{how}")),
        None => {}
    }
    if person.place != Place::Unknown {
        y += LINE * u;
    }
    if let Some(note) = &person.note {
        line(canvas, y, info, color::MUTED, format_args!("{note}"));
        y += LINE * u;
    }
    if !person.medals.is_empty() {
        for (index, (medal, _)) in person.medals.iter().take(MEDALS).enumerate() {
            let icon = Rect::new(x + index as f32 * 26.0 * u, y + 2.0 * u, 22.0 * u, 22.0 * u);
            let _ = canvas.draw_list_mut().push(DrawCommand::TexturedQuad {
                rect: icon,
                texture: medal.icon(),
                color: Color::new(1.0, 1.0, 1.0, 1.0),
            });
            crate::medals::rank::icon_device(
                canvas.draw_list_mut(),
                person.medals.rank(medal),
                icon,
            );
        }
        y += MEDAL_ROW * u;
    }
    y += 6.0 * u;
    // Whisper first, the whole row: what a click on a name is most often for.
    let whisper_button = Rect::new(x, y, inner, BUTTON * u);
    if let Some(token) = tokens.whisper {
        card_button(canvas, whisper_button, u, token, "Whisper", color::HOLO);
        y += (BUTTON + 6.0) * u;
    }
    let half = (inner - 8.0 * u) * 0.5;
    // Blocks for this map: the whole row, or two halves for staff, the right one for
    // everyone on the server.
    let map_button = Rect::new(
        x,
        y,
        if person.map_block.is_some_and(|row| row.staff.is_some()) {
            half
        } else {
            inner
        },
        BUTTON * u,
    );
    let staff_button = Rect::new(x + inner - half, y, half, BUTTON * u);
    if let Some(row) = person.map_block {
        let own = match (row.own, row.staff.is_some()) {
            (false, false) => "Block for this map",
            (true, false) => "Unblock this map",
            (false, true) => "Block this map",
            (true, true) => "Unblock map",
        };
        card_button(canvas, map_button, u, tokens.map_block, own, color::GOLD);
        if let Some(staff) = row.staff {
            let label = if staff {
                "Unblock all"
            } else {
                "Block for all"
            };
            card_button(
                canvas,
                staff_button,
                u,
                tokens.staff_block,
                label,
                color::EMBER,
            );
        }
        y += (BUTTON + 6.0) * u;
    }
    // The first row: Block on the left, and on the right Trade (when they can be asked)
    // or Ban 24 h: two even halves.
    let button = Rect::new(x, y, half, BUTTON * u);
    let right = Rect::new(x + inner - half, y, half, BUTTON * u);
    card_button(
        canvas,
        button,
        u,
        tokens.mute,
        if muted { "Unblock" } else { "Block" },
        color::GOLD,
    );
    if person.trade {
        card_button(canvas, right, u, tokens.trade, "Trade", color::GOLD);
    }
    let quiet = |muted: bool| {
        if muted {
            "Blocked here: no chat, Kyle, no sounds"
        } else {
            "Hides their chat, model, sounds and emotes"
        }
    };
    let under = y + (BUTTON + QUIET_GAP) * u;
    let small = 10.5 * u;
    // Ban 24 h for a sender the hub does not vouch for, when it vouches for the
    // player: beside Block, or on a row of its own under the quiet lines when Trade
    // takes that place. What it came to (or the armed button's warning) takes the quiet
    // line.
    let ban_key = ban_offered(person);
    let mut below = under
        + (QUIET_ROW
            + if person.key_id.is_none() {
                QUIET_ROW
            } else {
                0.0
            })
            * u;
    let ban_button = if person.trade {
        let row = Rect::new(x, below, inner, BUTTON * u);
        if ban_key.is_some() {
            below += (BUTTON + 6.0) * u;
        }
        row
    } else {
        right
    };
    match ban_key {
        Some(key) => crate::chat_ban::with_shown(key, |shown| {
            card_button(
                canvas,
                ban_button,
                u,
                tokens.ban,
                shown.label,
                if shown.armed {
                    color::EMBER
                } else {
                    color::HOLO
                },
            );
            let (said, colour) = match shown.line {
                Some((said, true)) => (said, color::GOLD_BRIGHT),
                Some((said, false)) => (said, color::EMBER),
                None => (quiet(muted), color::QUIET),
            };
            line(canvas, under, small, colour, format_args!("{said}"));
        }),
        None => line(
            canvas,
            under,
            small,
            color::QUIET,
            format_args!("{}", quiet(muted)),
        ),
    }
    if person.key_id.is_none() {
        // Without a key, the mute can only follow the name.
        line(
            canvas,
            under + QUIET_ROW * u,
            small,
            color::QUIET,
            format_args!("By name, colours aside: anyone wearing it"),
        );
    }
    // Safety code and Report share the last row, each the whole of it when alone.
    let (safety_width, report_x) = if person.safety && person.report {
        (half, x + inner - half)
    } else {
        (inner, x)
    };
    let safety_button = Rect::new(x, below, safety_width, BUTTON * u);
    let report_button = Rect::new(report_x, below, safety_width, BUTTON * u);
    if person.report {
        card_button(
            canvas,
            report_button,
            u,
            tokens.report,
            "Report",
            color::EMBER,
        );
    }
    if person.safety {
        card_button(
            canvas,
            safety_button,
            u,
            tokens.safety,
            "Safety code",
            crate::chat::encrypted_pink(1.0),
        );
    }
    canvas.hit_region(tokens.card, card);
    if let Some(token) = tokens.whisper {
        canvas.hit_region(token, whisper_button);
    }
    canvas.hit_region(tokens.mute, button);
    if let Some(row) = person.map_block {
        canvas.hit_region(tokens.map_block, map_button);
        if row.staff.is_some() {
            canvas.hit_region(tokens.staff_block, staff_button);
        }
    }
    if person.trade {
        canvas.hit_region(tokens.trade, right);
    }
    if ban_key.is_some() {
        canvas.hit_region(tokens.ban, ban_button);
    }
    if person.safety {
        canvas.hit_region(tokens.safety, safety_button);
    }
    if person.report {
        canvas.hit_region(tokens.report, report_button);
    }
    card
}

/// The key Ban 24 h is offered on, when it is ([`crate::chat_ban::offered`]).
fn ban_offered(person: &Person) -> Option<&str> {
    person.key_id.as_deref().filter(|key| {
        crate::chat_ban::offered(Some(key), person.verified, person.staff, person.trust)
    })
}

/// One of the card's buttons, outlined in `edge`, its fill lit under the pointer.
fn card_button(
    canvas: &mut MenuCanvas,
    button: Rect,
    u: f32,
    token: u16,
    label: &str,
    edge: Color,
) {
    let lit = canvas.token_hovered(token);
    let _ = canvas.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: button,
        radius: 6.0 * u,
        color: color::alpha(edge, if lit { 0.28 } else { 0.1 }),
    });
    let _ = canvas.draw_list_mut().push(DrawCommand::Border {
        rect: button,
        radius: 6.0 * u,
        width: 1.0 * u,
        color: color::alpha(edge, if lit { 1.0 } else { 0.7 }),
    });
    canvas.text_fmt_aligned(
        format_args!("{label}"),
        Rect::new(button.x, button.y + 6.5 * u, button.width, 17.0 * u),
        13.5 * u,
        if lit { color::GOLD_BRIGHT } else { color::TEXT },
        FontWeight::Semibold,
        0.0,
        TextAlign::Center,
    );
}

/// About how wide `name` is at `size` without a font at hand: a little over half the
/// size a character, colour codes taking no room.
fn estimated_width(name: &str, size: f32) -> f32 {
    let codes = name
        .as_bytes()
        .windows(2)
        .filter(|pair| pair[0] == b'^' && pair[1].is_ascii_digit())
        .count();
    name.chars().count().saturating_sub(codes * 2) as f32 * size * 0.55
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextStyle, test_font};

    const TOKENS: Tokens = Tokens {
        card: 7,
        mute: 8,
        ban: 9,
        safety: 10,
        report: 11,
        whisper: None,
        trade: 12,
        map_block: 13,
        staff_block: 14,
    };

    #[test]
    fn a_player_taking_encrypted_whispers_offers_the_safety_code() {
        let (canvas, _, texts, _) = drawn(&sol(), false);
        assert!(!texts.contains(&"Safety code".to_owned()), "{texts:?}");
        assert_eq!(canvas.rect_for(TOKENS.safety), None);
        let encrypted = Person {
            safety: true,
            ..sol()
        };
        let (canvas, rect, texts, _) = drawn(&encrypted, false);
        assert!(texts.contains(&"Safety code".to_owned()), "{texts:?}");
        let button = canvas.rect_for(TOKENS.safety).expect("the button");
        let block = canvas.rect_for(TOKENS.mute).expect("Block");
        assert!(button.y > block.bottom(), "under the other buttons");
        assert!(button.bottom() <= rect.bottom(), "inside the card");
    }

    #[test]
    fn a_whisper_that_came_can_be_reported_beside_the_safety_code() {
        let report = Person {
            report: true,
            ..sol()
        };
        let (canvas, rect, texts, _) = drawn(&report, false);
        assert!(texts.contains(&"Report".to_owned()), "{texts:?}");
        let alone = canvas.rect_for(TOKENS.report).expect("Report");
        assert!(alone.bottom() <= rect.bottom(), "inside the card");
        let both = Person {
            safety: true,
            ..report
        };
        let (canvas, rect, _, _) = drawn(&both, false);
        let safety = canvas.rect_for(TOKENS.safety).expect("Safety code");
        let report = canvas.rect_for(TOKENS.report).expect("Report");
        assert_eq!(safety.y, report.y, "one row");
        assert!(safety.right() < report.x, "side by side");
        assert!(report.width < alone.width && report.bottom() <= rect.bottom());
    }

    #[test]
    fn a_player_who_can_trade_is_offered_trade_beside_block() {
        let (canvas, ..) = drawn(&sol(), false);
        assert_eq!(canvas.rect_for(TOKENS.trade), None);
        for safety in [false, true] {
            let trader = Person {
                trade: true,
                safety,
                ..sol()
            };
            let (canvas, rect, texts, _) = drawn(&trader, false);
            assert!(texts.contains(&"Trade".to_owned()), "{texts:?}");
            let trade = canvas.rect_for(TOKENS.trade).expect("Trade");
            let block = canvas.rect_for(TOKENS.mute).expect("Block");
            // Beside Block, as wide, the gap between them centred in the card.
            assert_eq!(
                (trade.y, trade.width, trade.height),
                (block.y, block.width, block.height)
            );
            let gap = trade.x - block.right();
            assert!(gap > 0.0 && (block.x - rect.x - (rect.right() - trade.right())).abs() < 0.5);
            assert!(trade.bottom() <= rect.bottom());
            if let Some(code) = canvas.rect_for(TOKENS.safety) {
                assert!(code.y >= trade.bottom() && code.bottom() <= rect.bottom());
            }
        }
    }

    #[test]
    fn the_status_line_names_staff_and_the_creator() {
        let staff = Person {
            staff: true,
            ..sol()
        };
        assert_eq!(staff.status(), "SJK player, SJK staff");
        let creator = Person {
            staff: true,
            creator: true,
            ..sol()
        };
        assert_eq!(creator.status(), "SJK player, SJK creator");
        assert_eq!(sol().status(), "SJK player");
    }

    fn sol() -> Person {
        Person {
            name: "^2Sol".to_owned(),
            key_id: Some("0123456789abcdef".to_owned()),
            hub_name: None,
            verified: true,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            medals: crate::medals::Medals::default(),
            place: Place::SlotByName(5),
            banner: "",
            avatar: Some(String::new()),
            note: None,
            safety: false,
            report: false,
            trade: false,
            map_block: None,
        }
    }

    #[test]
    fn a_player_on_the_server_can_be_blocked_for_the_map_and_by_staff_for_all() {
        let (canvas, _, texts, _) = drawn(&sol(), false);
        assert!(
            canvas.rect_for(TOKENS.map_block).is_none(),
            "not on the server"
        );
        assert!(!texts.iter().any(|text| text.contains("this map")));
        let player = Person {
            map_block: Some(MapBlockRow::default()),
            ..sol()
        };
        let (canvas, rect, texts, _) = drawn(&player, false);
        let map = canvas
            .rect_for(TOKENS.map_block)
            .expect("Block for this map");
        assert!(texts.iter().any(|text| text == "Block for this map"));
        assert!(canvas.rect_for(TOKENS.staff_block).is_none(), "staff only");
        assert!(map.right() <= rect.right());
        let staff = Person {
            map_block: Some(MapBlockRow {
                own: true,
                staff: Some(false),
            }),
            ..sol()
        };
        let (canvas, rect, texts, _) = drawn(&staff, false);
        for wanted in ["Unblock map", "Block for all"] {
            assert!(texts.iter().any(|text| text == wanted), "{wanted}");
        }
        let (map, all) = (
            canvas.rect_for(TOKENS.map_block).unwrap(),
            canvas.rect_for(TOKENS.staff_block).unwrap(),
        );
        assert!(map.right() <= all.x && all.right() <= rect.right() && map.y == all.y);
        let block = canvas.rect_for(TOKENS.mute).expect("Block");
        assert!(map.bottom() <= block.y, "above Block");
    }

    /// The card drawn at (100, 100) on a 1080p canvas: its texts and whether it has
    /// the verified tick.
    fn drawn(person: &Person, muted: bool) -> (MenuCanvas, Rect, Vec<String>, usize) {
        let font = test_font();
        let measure = crate::sjk_chat_look::Measure::new(&font, TextStyle::NEUTRAL);
        let mut canvas = MenuCanvas::new();
        canvas.begin_transparent([1920.0, 1080.0]);
        let rect = draw(
            &mut canvas,
            &Card {
                person,
                muted,
                measure: Some(&measure),
            },
            [100.0, 100.0],
            1.0,
            TOKENS,
        );
        let texts = canvas.text_runs().map(str::to_owned).collect();
        let ticks = canvas
            .draw_list()
            .commands()
            .iter()
            .filter(|command| {
                matches!(command, DrawCommand::TexturedQuad { texture, .. }
                    if *texture == crate::ui_renderer::VERIFIED_TEXTURE)
            })
            .count();
        (canvas, rect, texts, ticks)
    }

    #[test]
    fn the_card_says_who_they_are_and_offers_mute() {
        let (canvas, rect, texts, ticks) = drawn(&sol(), false);
        assert!(texts.contains(&"^2Sol".to_owned()), "{texts:?}");
        assert!(texts.contains(&"SJK player".to_owned()), "{texts:?}");
        assert!(
            !texts.iter().any(|text| text.contains("0123456789abcdef")),
            "key ids never show: {texts:?}"
        );
        assert!(
            texts.contains(&"On this server, slot 5 (by name)".to_owned()),
            "{texts:?}"
        );
        assert!(texts.contains(&"Block".to_owned()), "{texts:?}");
        assert_eq!(ticks, 1, "the tick alone says verified");
        assert!(
            !texts.iter().any(|text| text.contains("erified")),
            "{texts:?}"
        );
        // No picture (or not loaded): their initial in its disc, left of the name.
        let at = |wanted: &str| {
            canvas
                .draw_list()
                .commands()
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Text { rect, text, .. } if canvas.stored_text(*text) == wanted => {
                        Some(*rect)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{wanted:?} in {texts:?}"))
        };
        let (initial, name) = (at("S"), at("^2Sol"));
        assert!(initial.right() <= name.x && initial.y <= name.y + name.height);
        // Both targets: the card, and the button inside it.
        assert_eq!(canvas.rect_for(TOKENS.card), Some(rect));
        let button = canvas.rect_for(TOKENS.mute).expect("the button");
        assert!(rect.contains(sjk_ui::Vec2::new(button.x + 1.0, button.y + 1.0)));
        assert!(button.bottom() <= rect.bottom());
    }

    /// A note from the place that shows the card (the chat page's window: when they
    /// were about) takes a line of its own, inside the card.
    #[test]
    fn a_note_gets_its_line_and_the_card_grows_for_it() {
        let plain = sol();
        let noted = Person {
            note: Some("Seen 3 minutes ago".to_owned()),
            ..sol()
        };
        let (_, rect, texts, _) = drawn(&noted, false);
        assert!(
            texts.contains(&"Seen 3 minutes ago".to_owned()),
            "{texts:?}"
        );
        assert_eq!(
            size(&noted, false, 1.0)[1],
            size(&plain, false, 1.0)[1] + LINE
        );
        assert_eq!(rect.height, size(&noted, false, 1.0)[1]);
        let (_, _, texts, _) = drawn(&plain, false);
        assert!(!texts.iter().any(|text| text.starts_with("Seen")));
    }

    #[test]
    fn a_muted_player_is_offered_unmute_and_a_stranger_says_so() {
        let stranger = Person {
            name: "Fox".to_owned(),
            place: Place::Slot(3),
            ..Person::default()
        };
        let (_, _, texts, ticks) = drawn(&stranger, true);
        assert!(texts.contains(&"Unblock".to_owned()), "{texts:?}");
        assert!(texts.contains(&"F".to_owned()), "their initial: {texts:?}");
        assert!(
            texts.contains(&"Not known to the SJK hub".to_owned()),
            "{texts:?}"
        );
        assert!(
            texts.contains(&"On this server, slot 3".to_owned()),
            "{texts:?}"
        );
        assert!(
            texts.contains(&"By name, colours aside: anyone wearing it".to_owned()),
            "{texts:?}"
        );
        assert_eq!(ticks, 0);
    }

    /// Ban 24 h sits beside Mute only for a player the hub vouches for, on a sender
    /// with a key it does not vouch for.
    #[test]
    fn ban_shows_beside_mute_only_when_offered() {
        let unverified = Person {
            verified: false,
            ..sol()
        };
        let (canvas, rect, texts, _) = drawn(&unverified, false);
        assert!(
            !texts.contains(&crate::chat_ban::BAN.to_owned()),
            "{texts:?}"
        );
        assert!(canvas.rect_for(TOKENS.ban).is_none());
        crate::chat_ban::trust_for_test(true);
        let (canvas, _, texts, _) = drawn(&unverified, false);
        assert!(
            texts.contains(&crate::chat_ban::BAN.to_owned()),
            "{texts:?}"
        );
        let ban = canvas.rect_for(TOKENS.ban).expect("Ban 24 h");
        let mute = canvas.rect_for(TOKENS.mute).expect("Mute");
        assert!(ban.x >= mute.right() && ban.right() <= rect.right() && ban.y == mute.y);
        // Not on a verified sender, a staff one, or one without a key.
        for person in [
            sol(),
            Person {
                staff: true,
                ..unverified.clone()
            },
            Person {
                key_id: None,
                ..unverified.clone()
            },
        ] {
            let (canvas, ..) = drawn(&person, false);
            assert!(canvas.rect_for(TOKENS.ban).is_none(), "{person:?}");
        }
        crate::chat_ban::trust_for_test(false);
    }

    #[test]
    fn the_card_goes_beside_the_name_and_stays_on_screen() {
        let viewport = [1920.0, 1080.0];
        let size = size(&sol(), false, 1.0);
        let left_name = Rect::new(100.0, 500.0, 80.0, 20.0);
        assert_eq!(beside(left_name, size, viewport, 10.0), [190.0, 500.0]);
        // At the right edge it goes to the name's left.
        let right_name = Rect::new(1800.0, 500.0, 80.0, 20.0);
        let [x, _] = beside(right_name, size, viewport, 10.0);
        assert!(x + size[0] <= 1790.0 + 0.01);
        // Low on the screen it is raised to fit.
        let low = Rect::new(100.0, 1070.0, 80.0, 20.0);
        let [_, y] = beside(low, size, viewport, 10.0);
        assert!(y + size[1] <= 1080.0 + 0.01);
    }
}
