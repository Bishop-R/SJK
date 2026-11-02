//! The markup an announcement's text may carry (`PROTOCOL.md` in Sol-Vulpes/SJK-hub,
//! "Announcements", 31/10/2026; `docs/hub-chat.md`, "News panel"): links into the
//! client's UI, pictures and switches. Only the SJK team writes announcements (the hub
//! refuses anyone else), so only they make these; chat messages, bios and names never
//! read them.
//!
//! - A link, inline: `[Label](target)`, the target `setting:<cvar>` (Settings searched
//!   for that setting) or `open:<page>` (a page of the client's table, `ui_links.rs` in
//!   the viewer). A label is 1 to [`LABEL_MAX`] characters without brackets.
//! - A picture, on its own line: `[[image:<id>]]`, a news picture the hub holds (16
//!   lowercase hex digits), at most [`IMAGES_MAX`].
//! - A switch, on its own line: `[[switch:<cvar>]]`, the Settings on/off row of that
//!   setting, at most [`SWITCHES_MAX`].
//!
//! A `[` that starts something that looks like a link (`](` follows it) but is not a
//! well-formed one is refused by [`check`], so a typo is caught when sending, not shown
//! raw; a plain `[word]` stays text.

/// Longest link label, in characters.
pub const LABEL_MAX: usize = 60;
/// Most pictures one text shows.
pub const IMAGES_MAX: usize = 6;
/// Most switches one text holds.
pub const SWITCHES_MAX: usize = 9;

/// A piece of a line of text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Span<'a> {
    /// Plain words (backticks included: the client draws code in chips).
    Text(&'a str),
    /// A link: its label and target.
    Link {
        /// What the link reads.
        label: &'a str,
        /// Where it leads ([`Target::parse`]).
        target: &'a str,
    },
}

/// One line of a text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Line<'a> {
    /// A blank line: a paragraph break.
    Blank,
    /// `[[image:<id>]]`.
    Image(&'a str),
    /// `[[switch:<cvar>]]`.
    Switch(&'a str),
    /// A line of words; `bullet` when it starts with `- ` (the dash left out).
    Text {
        /// The line's words.
        text: &'a str,
        /// It started with `- `.
        bullet: bool,
    },
}

/// Where a link's target leads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target<'a> {
    /// `setting:<cvar>`.
    Setting(&'a str),
    /// `open:<page>`.
    Open(&'a str),
}

impl<'a> Target<'a> {
    /// The target `raw` names, when it is well formed.
    pub fn parse(raw: &'a str) -> Option<Self> {
        if let Some(cvar) = raw.strip_prefix("setting:") {
            return cvar_ok(cvar).then_some(Self::Setting(cvar));
        }
        let page = raw.strip_prefix("open:")?;
        let ok = (1..=32).contains(&page.len())
            && page
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        ok.then_some(Self::Open(page))
    }
}

/// Whether `cvar` reads as a cvar name: 1 to 64 letters, digits, `_` and `.`.
pub fn cvar_ok(cvar: &str) -> bool {
    (1..=64).contains(&cvar.len())
        && cvar
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

/// Whether `id` reads as a news picture's id: 16 lowercase hex digits.
pub fn image_id_ok(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What line `raw` is.
pub fn line(raw: &str) -> Line<'_> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Line::Blank;
    }
    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|rest| rest.strip_suffix("]]"))
    {
        if let Some(id) = inner.strip_prefix("image:")
            && image_id_ok(id)
        {
            return Line::Image(id);
        }
        if let Some(cvar) = inner.strip_prefix("switch:")
            && cvar_ok(cvar)
        {
            return Line::Switch(cvar);
        }
    }
    match trimmed.strip_prefix("- ") {
        Some(text) => Line::Text {
            text: text.trim_start(),
            bullet: true,
        },
        None => Line::Text {
            text: trimmed,
            bullet: false,
        },
    }
}

/// The link starting at the start of `text` (`[Label](target)`), with its length in
/// bytes, when it is well formed.
fn link_at(text: &str) -> Option<(&str, &str, usize)> {
    let rest = text.strip_prefix('[')?;
    let close = rest.find(']')?;
    let label = &rest[..close];
    let after = rest[close + 1..].strip_prefix('(')?;
    let end = after.find(')')?;
    let target = &after[..end];
    let label_ok = (1..=LABEL_MAX).contains(&label.chars().count())
        && !label.trim().is_empty()
        && !label.contains(['[', ']', '(', ')', '\n']);
    (label_ok && Target::parse(target).is_some()).then(|| {
        let length = 1 + close + 1 + 1 + end + 1;
        (label.trim(), target, length)
    })
}

/// `text`'s pieces: plain text and well-formed links. Anything else stays text.
pub fn spans(text: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut plain_from = 0;
    let mut at = 0;
    while let Some(offset) = text[at..].find('[') {
        let start = at + offset;
        match link_at(&text[start..]) {
            Some((label, target, length)) => {
                if start > plain_from {
                    spans.push(Span::Text(&text[plain_from..start]));
                }
                spans.push(Span::Link { label, target });
                at = start + length;
                plain_from = at;
            }
            None => at = start + 1,
        }
    }
    if plain_from < text.len() {
        spans.push(Span::Text(&text[plain_from..]));
    }
    spans
}

/// `text` as a surface without links or pictures shows it: each link its label, picture
/// and switch lines left out (a blank line kept in their place once).
pub fn plain(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        match line(raw) {
            Line::Image(_) | Line::Switch(_) | Line::Blank => {
                if out.last().is_some_and(|last| !last.is_empty()) {
                    out.push(String::new());
                }
            }
            Line::Text { .. } => out.push(
                spans(raw)
                    .iter()
                    .map(|span| match span {
                        Span::Text(text) => *text,
                        Span::Link { label, .. } => label,
                    })
                    .collect(),
            ),
        }
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

/// Which markup rule a text breaks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkupError {
    /// `[`...`](` that is not a well-formed link: the line's number (from 1).
    Link(usize),
    /// A `[[...]]` line that is neither a picture nor a switch: the line's number.
    Block(usize),
    /// A picture the hub does not hold: its id.
    UnknownImage(String),
    /// More than [`IMAGES_MAX`] pictures.
    TooManyImages,
    /// More than [`SWITCHES_MAX`] switches.
    TooManySwitches,
}

impl std::fmt::Display for MarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(line) => write!(
                f,
                "Line {line}: a link is [Label](setting:<cvar>) or [Label](open:<page>), \
                 its label 1 to {LABEL_MAX} characters."
            ),
            Self::Block(line) => write!(
                f,
                "Line {line}: on its own line, [[image:<id>]] or [[switch:<cvar>]]."
            ),
            Self::UnknownImage(id) => write!(f, "No uploaded picture {id}."),
            Self::TooManyImages => write!(f, "At most {IMAGES_MAX} pictures."),
            Self::TooManySwitches => write!(f, "At most {SWITCHES_MAX} switches."),
        }
    }
}

/// The first markup rule `text` breaks, `known_image` telling the pictures the hub holds.
pub fn check(text: &str, known_image: impl Fn(&str) -> bool) -> Result<(), MarkupError> {
    let (mut images, mut switches) = (0, 0);
    for (index, raw) in text.split('\n').enumerate() {
        let number = index + 1;
        match line(raw) {
            Line::Blank => {}
            Line::Image(id) => {
                if !known_image(id) {
                    return Err(MarkupError::UnknownImage(id.to_owned()));
                }
                images += 1;
            }
            Line::Switch(_) => switches += 1,
            Line::Text { text, .. } => {
                if text.starts_with("[[") {
                    return Err(MarkupError::Block(number));
                }
                // Every `[` followed (before the next `[`) by `](` must open a link.
                let mut at = 0;
                while let Some(offset) = text[at..].find('[') {
                    let start = at + offset;
                    let next = text[start + 1..]
                        .find('[')
                        .map_or(text.len(), |n| start + 1 + n);
                    let looks = text[start..next].contains("](");
                    match link_at(&text[start..]) {
                        Some((_, _, length)) => at = start + length,
                        None if looks => return Err(MarkupError::Link(number)),
                        None => at = start + 1,
                    }
                }
            }
        }
    }
    if images > IMAGES_MAX {
        return Err(MarkupError::TooManyImages);
    }
    if switches > SWITCHES_MAX {
        return Err(MarkupError::TooManySwitches);
    }
    Ok(())
}

/// The pictures `text` names, in order.
pub fn images(text: &str) -> Vec<&str> {
    text.split('\n')
        .filter_map(|raw| match line(raw) {
            Line::Image(id) => Some(id),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef";

    #[test]
    fn links_are_cut_out_of_their_line() {
        assert_eq!(
            spans("Open [the Sith cloud](open:sith) or [Glow](setting:r_dynamicGlow)."),
            [
                Span::Text("Open "),
                Span::Link {
                    label: "the Sith cloud",
                    target: "open:sith"
                },
                Span::Text(" or "),
                Span::Link {
                    label: "Glow",
                    target: "setting:r_dynamicGlow"
                },
                Span::Text("."),
            ]
        );
        assert_eq!(spans("a [word] stays"), [Span::Text("a [word] stays")]);
        assert_eq!(
            spans("[x](https://example.com)"),
            [Span::Text("[x](https://example.com)")],
            "only the client's own places"
        );
    }

    #[test]
    fn targets_name_a_setting_or_a_page() {
        assert_eq!(
            Target::parse("setting:cg_nameplate"),
            Some(Target::Setting("cg_nameplate"))
        );
        assert_eq!(
            Target::parse("open:jof.downloads"),
            None,
            "pages are lowercase words"
        );
        assert_eq!(Target::parse("open:sith"), Some(Target::Open("sith")));
        for bad in [
            "",
            "open:",
            "setting:",
            "run:quit",
            "setting:a b",
            "open:Sith",
        ] {
            assert_eq!(Target::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn picture_switch_and_bullet_lines() {
        assert_eq!(line(&format!(" [[image:{ID}]] ")), Line::Image(ID));
        assert_eq!(
            line("[[switch:cg_nameplate]]"),
            Line::Switch("cg_nameplate")
        );
        assert_eq!(
            line("- a point"),
            Line::Text {
                text: "a point",
                bullet: true
            }
        );
        assert_eq!(line("  "), Line::Blank);
        assert!(matches!(line("[[image:XYZ]]"), Line::Text { .. }));
    }

    #[test]
    fn the_plain_form_keeps_labels_and_drops_pictures() {
        let text = format!(
            "See [the cloud](open:sith).\n[[image:{ID}]]\nThen [[x]] stays.\n[[switch:cg_a]]"
        );
        assert_eq!(plain(&text), "See the cloud.\n\nThen [[x]] stays.");
    }

    #[test]
    fn the_check_catches_broken_links_and_unknown_pictures() {
        let known = |id: &str| id == ID;
        assert_eq!(check("Fine [a](open:sith) and [b]", known), Ok(()));
        assert_eq!(
            check("x\n[bad](run:quit)", known),
            Err(MarkupError::Link(2))
        );
        assert_eq!(check("[](open:sith)", known), Err(MarkupError::Link(1)));
        assert_eq!(
            check(&format!("[{}](open:sith)", "a".repeat(61)), known),
            Err(MarkupError::Link(1))
        );
        assert_eq!(check("[[nothing]]", known), Err(MarkupError::Block(1)));
        assert_eq!(
            check("[[image:ffffffffffffffff]]", known),
            Err(MarkupError::UnknownImage("ffffffffffffffff".to_owned()))
        );
        let seven = vec![format!("[[image:{ID}]]"); 7].join("\n");
        assert_eq!(check(&seven, known), Err(MarkupError::TooManyImages));
        let ten = ["[[switch:cg_a]]"; 10].join("\n");
        assert_eq!(check(&ten, known), Err(MarkupError::TooManySwitches));
        assert_eq!(images(&format!("a\n[[image:{ID}]]")), [ID]);
    }
}
