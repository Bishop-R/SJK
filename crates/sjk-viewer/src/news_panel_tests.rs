use super::*;
use crate::sjk_chat_look::Measure;
use crate::text::{TextStyle, test_font};

fn digest(title: &str, date: &str, lines: &[(&str, &str)]) -> Digest {
    Digest {
        title: title.to_owned(),
        date: date.to_owned(),
        intro: "A release introduction that is not a recap.".to_owned(),
        lines: lines
            .iter()
            .map(|(heading, sentence)| ((*heading).to_owned(), (*sentence).to_owned()))
            .collect(),
        people: vec!["Sol".to_owned(), "Creyon".to_owned()],
        after: vec!["JoF EJK".to_owned()],
    }
}

fn review(cvar: &str, reset: bool) -> Review {
    Review {
        cvar: cvar.to_owned(),
        reset,
        text: "Why it is asked.".to_owned(),
    }
}

fn announcement(
    id: u64,
    at: i64,
    kind: &str,
    title: &str,
    text: &str,
    version: &str,
) -> Announcement {
    Announcement {
        id,
        at,
        kind: kind.to_owned(),
        title: title.to_owned(),
        text: text.to_owned(),
        version: version.to_owned(),
        images: BTreeMap::new(),
    }
}

/// Lay the selected entry out with test glyphs, 12 frame pixels a character at the
/// text size.
fn laid_out(panel: &mut Panel) {
    let font = test_font();
    let measure = Measure::new(&font, TextStyle::NEUTRAL);
    panel.prepare(&Measures {
        body: measure,
        code: measure,
    });
}

fn sources<'a>(
    digests: &'a [Digest],
    reviews: &'a [Review],
    announcements: &'a [Announcement],
) -> Sources<'a> {
    Sources {
        digests,
        reviews,
        announcements,
        new_releases: &[],
        read: 0,
        installed: "2026.1031.1",
    }
}

#[test]
fn dates_read_as_days_in_order() {
    let a = date_seconds("31/10/2026").unwrap();
    let b = date_seconds("01/11/2026").unwrap();
    assert_eq!(b - a, 86_400);
    // 31/10/2026 12:00 UTC.
    assert_eq!(a, 1_793_448_000);
    assert_eq!(date_seconds(""), None);
    assert_eq!(date_seconds("32/01/2026"), None);
}

/// Releases and announcements share one list, newest first, with the settings to
/// review on top; a release's announcement goes on its release's entry, above its
/// recap.
#[test]
fn releases_and_announcements_make_one_list() {
    let digests = [
        digest(
            "2026.1031.1 (Alpha)",
            "31/10/2026",
            &[("Servers", "Faster.")],
        ),
        digest("2026.1030.1 (Alpha)", "30/10/2026", &[("HUD", "Clock.")]),
    ];
    let night = date_seconds("30/10/2026").unwrap() + 3_600 * 8;
    let list = [
        announcement(7, night, "news", "Holocrons tonight", "Come and play.", ""),
        announcement(
            8,
            date_seconds("31/10/2026").unwrap() + 60,
            "release",
            "SJK 2026.1031.1",
            "A big one.",
            "2026.1031.1",
        ),
    ];
    let reviews = [
        review("cg_nameplate", false),
        review("not_a_setting", false),
    ];
    let entries = entries(&sources(&digests, &reviews, &list));
    let keys: Vec<&Key> = entries.iter().map(|entry| &entry.key).collect();
    assert_eq!(
        keys,
        [
            &Key::Review,
            &Key::Release("2026.1031.1 (Alpha)".to_owned()),
            &Key::Announcement(7),
            &Key::Release("2026.1030.1 (Alpha)".to_owned()),
        ]
    );
    // One switch: Settings has no row for the other.
    assert_eq!(entries[0].blocks.len(), 1);
    assert_eq!(
        entries[1].blocks[..3],
        [
            Block::Text("A big one.".to_owned(), false),
            Block::Gap,
            Block::Line("Servers".to_owned(), "Faster.".to_owned()),
        ]
    );
    assert!(entries[1].unread, "the release's announcement is new");
    assert!(!entries[3].unread);
    assert_eq!(entries[2].kicker, "FROM THE SJK TEAM");
    assert!(!entries[2].update);
}

/// A release announced for a version this build does not list offers the update.
#[test]
fn a_newer_release_offers_the_update() {
    let list = [announcement(
        9,
        10,
        "release",
        "SJK 2026.1101.1",
        "New.",
        "2026.1101.1",
    )];
    let entries = entries(&sources(&[], &[], &list));
    assert_eq!(entries[0].key, Key::Announcement(9));
    assert!(entries[0].update);
    let mut panel = Panel::default();
    panel.offer(entries, None, true, "");
    assert_eq!(panel.key(KeyCode::KeyU), Some(Choice::Update));
}

/// An announcement reads as paragraphs, bullets, pictures and switches; a switch
/// Settings has no on/off row for is left out.
#[test]
fn an_announcements_lines_become_blocks() {
    let text = "Hello [the cloud](open:sith).\n\n- one\n- two\n[[image:0123456789abcdef]]\n[[switch:cg_nameplate]]\n[[switch:nope]]";
    assert_eq!(
        announcement_blocks(text),
        [
            Block::Text("Hello [the cloud](open:sith).".to_owned(), false),
            Block::Gap,
            Block::Text("one".to_owned(), true),
            Block::Text("two".to_owned(), true),
            Block::Image("0123456789abcdef".to_owned()),
            Block::Setting(
                "cg_nameplate".to_owned(),
                crate::settings::switch_label("cg_nameplate").unwrap(),
                String::new()
            ),
        ]
    );
}

/// Links and chips answer in reading order, and the pane knows a link to a place this
/// build has; a link to a place it does not know reads as its label.
#[test]
fn links_and_chips_are_laid_out_whole() {
    let mut list = [announcement(
        3,
        5,
        "news",
        "Clouds",
        "Open [the Sith cloud](open:sith) or type `news`, not [this](open:nowhere).",
        "",
    )];
    list[0].text = list[0].text.replace("open:sith", "open:collection");
    let mut panel = Panel::default();
    panel.offer(entries(&sources(&[], &[], &list)), None, true, "");
    laid_out(&mut panel);
    let actions: Vec<Action> = panel
        .laid
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Row { row, .. } => {
                Some(inline_code::actions(std::slice::from_ref(row)).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        actions,
        [
            Action::Link("open:collection".to_owned()),
            Action::Chip("news".to_owned())
        ]
    );
    let words: String = panel
        .laid
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Row { row, .. } => Some(
                row.iter()
                    .map(|run| run.text.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(words.contains("this"), "{words}");
    assert!(!words.contains("open:nowhere"), "{words}");
}

/// A picture is laid out at its size, kept to the pane and to its tallest.
#[test]
fn pictures_keep_their_shape_within_the_pane() {
    let id = "0123456789abcdef";
    let mut list = [announcement(
        4,
        5,
        "news",
        "Look",
        &format!("See:\n[[image:{id}]]"),
        "",
    )];
    list[0].images.insert(id.to_owned(), [1600, 900]);
    let mut panel = Panel::default();
    panel.offer(entries(&sources(&[], &[], &list)), None, true, "");
    laid_out(&mut panel);
    let Some(Item::Image(laid_id, [width, height])) = panel
        .laid
        .items
        .iter()
        .find(|item| matches!(item, Item::Image(..)))
    else {
        panic!("{:?}", panel.laid.items);
    };
    assert_eq!(laid_id, id);
    assert!(*width <= PANE_WIDTH && *height <= IMAGE_MAX_HEIGHT);
    assert!((width / height - 16.0 / 9.0).abs() < 0.01);
}

/// The panel shows until it is closed; other keys are swallowed.
#[test]
fn the_panel_shows_until_it_is_closed() {
    let digests = [digest(
        "2.0",
        "12/10/2026",
        &[("Servers", "Downloads work.")],
    )];
    let mut panel = Panel::default();
    assert!(!panel.wants());
    panel.offer(Vec::new(), None, false, "2.0");
    assert!(!panel.wants(), "nothing to show");
    panel.offer(entries(&sources(&digests, &[], &[])), None, false, "2.0");
    assert!(panel.wants());
    assert_eq!(panel.version(), "2.0");
    assert_eq!(panel.key(KeyCode::KeyA), None);
    assert!(panel.wants(), "other keys are swallowed");
    assert_eq!(panel.key(KeyCode::Escape), Some(Choice::Close));
    assert!(!panel.wants());
    panel.offer(entries(&sources(&digests, &[], &[])), None, true, "");
    assert!(panel.asked());
    assert_eq!(panel.key(KeyCode::KeyC), Some(Choice::Changelog));
    assert!(!panel.asked(), "closing forgets it was asked for");
}

/// The settings to review come first; a number flips one and leaves the panel up.
#[test]
fn switches_flip_without_closing_and_keep_the_lowercase_name() {
    let digests = [digest("2.0", "12/10/2026", &[("Servers", "x")])];
    let reviews = [review("CG_Nameplate", true)];
    let mut panel = Panel::default();
    panel.offer(
        entries(&sources(&digests, &reviews, &[])),
        None,
        false,
        "2.0",
    );
    laid_out(&mut panel);
    assert_eq!(panel.selected_key(), Some(&Key::Review));
    assert_eq!(panel.switch_cvars(), ["cg_nameplate"]);
    panel.sync_switches(|cvar| cvar == "cg_nameplate");
    assert_eq!(panel.on, [true]);
    assert_eq!(
        panel.key(KeyCode::Digit1),
        Some(Choice::Switch("cg_nameplate".to_owned()))
    );
    assert_eq!(panel.key(KeyCode::Digit2), None, "only the switches shown");
    assert!(panel.wants(), "switching keeps the panel");
    // Down shows the release, which has no switches.
    assert_eq!(panel.key(KeyCode::ArrowDown), None);
    laid_out(&mut panel);
    assert_eq!(panel.selected_key(), Some(&Key::Release("2.0".to_owned())));
    assert_eq!(panel.key(KeyCode::Digit1), None);
}

/// A number cvar's switch flips both ways and reads as its Settings row does (Sol's
/// report, 31/10/2026: the switch did nothing, as it only read true-or-false cvars).
#[test]
fn a_number_switch_flips_on_and_off() {
    let directory = tempfile::tempdir().unwrap();
    let mut console =
        crate::console::ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
    let cvar = crate::frame_target::aa::glow::NO_SABERS_CVAR.to_ascii_lowercase();
    assert!(!switch_on(&console, &cvar));
    flip(&mut console, &cvar);
    assert!(switch_on(&console, &cvar));
    assert_eq!(console.integer_cvar(&cvar), Some(1));
    flip(&mut console, &cvar);
    assert!(!switch_on(&console, &cvar));
}

/// A reset setting goes back to its default; one only asked about keeps its value.
#[test]
fn reset_settings_go_back_to_their_default_and_the_others_stay() {
    let directory = tempfile::tempdir().unwrap();
    let mut console =
        crate::console::ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
    console.set_cvar("cg_nameplate", "1");
    console.set_cvar("cg_nameplateIcons", "0");
    reset_reviewed(
        &mut console,
        &[
            review("cg_nameplate", true),
            review("cg_nameplateIcons", false),
            review("not_a_setting", true),
        ],
    );
    assert_eq!(console.bool_cvar("cg_nameplate"), Some(false));
    assert_eq!(console.bool_cvar("cg_nameplateIcons"), Some(false));
}

/// The reset applies once per version: a later start with the panel still unanswered,
/// after the player turned the setting on again, keeps it on.
#[test]
fn a_reset_applies_once_per_version() {
    let directory = tempfile::tempdir().unwrap();
    let mut console =
        crate::console::ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
    let resets = |installed: &str, _: &str, applied: &str| {
        if applied == installed {
            Vec::new()
        } else {
            vec![review("cg_nameplate", true)]
        }
    };
    console.set_cvar("cg_nameplate", "1");
    reset_once(&mut console, "2.0", "1.0", resets);
    assert_eq!(console.bool_cvar("cg_nameplate"), Some(false));
    assert_eq!(console.text_cvar(RESET_CVAR).ok(), Some("2.0"));
    console.set_cvar("cg_nameplate", "1");
    reset_once(&mut console, "2.0", "1.0", resets);
    assert_eq!(
        console.bool_cvar("cg_nameplate"),
        Some(true),
        "not reset again"
    );
}

#[test]
fn the_marker_is_read_once_a_run() {
    let mut panel = Panel::default();
    assert!(panel.take_check());
    assert!(!panel.take_check());
}

/// A long text scrolls by its lines, never past its last one.
#[test]
fn a_long_entry_scrolls_to_its_end() {
    let long = "word ".repeat(60);
    let lines: Vec<(&str, &str)> = (0..12).map(|_| ("Heading", long.as_str())).collect();
    let digests = [digest("3.0", "12/10/2026", &lines)];
    let mut panel = Panel::default();
    panel.offer(entries(&sources(&digests, &[], &[])), None, true, "");
    laid_out(&mut panel);
    let last = max_scroll(&panel.laid.items);
    assert!(last > 0);
    for _ in 0..50 {
        panel.key(KeyCode::PageDown);
    }
    assert_eq!(panel.scroll, last);
    let rest: f32 = panel.laid.items[last..].iter().map(Item::height).sum();
    assert!(rest <= BODY_BOTTOM - BODY_TOP);
    panel.key(KeyCode::PageUp);
    assert!(panel.scroll < last);
}

/// A missing recap falls back to the introduction, cut with an ellipsis.
#[test]
fn a_missing_recap_falls_back_to_the_introduction() {
    let long = Digest {
        intro: "word ".repeat(400),
        ..digest("1.0", "12/10/2026", &[])
    };
    let mut panel = Panel::default();
    panel.offer(entries(&sources(&[long], &[], &[])), None, true, "");
    laid_out(&mut panel);
    let rows: Vec<&Item> = panel
        .laid
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                Item::Row {
                    tone: Tone::Muted,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(rows.len(), INTRO_LINES);
    let Item::Row { row, .. } = rows[INTRO_LINES - 1] else {
        unreachable!()
    };
    assert!(row.last().unwrap().text.ends_with("..."));
}

#[test]
fn headings_find_their_icons_and_every_icon_exists() {
    assert_eq!(heading_icon("Browser"), "network");
    assert_eq!(heading_icon("Saber shaders"), "saber_style");
    assert_eq!(heading_icon("Small fixes"), "options");
    assert_eq!(heading_icon("Gameplay FPS"), "gameplay");
    assert_eq!(heading_icon("HUD"), "hud");
    // A word inside another is no match ("ui" in "build").
    assert_eq!(heading_icon("Builds"), HEADING_ICON);
    for (_, icon) in HEADING_ICONS {
        assert!(crate::settings_icons::texture(icon).is_some(), "{icon}");
    }
    assert!(crate::settings_icons::texture(HEADING_ICON).is_some());
    assert!(crate::settings_icons::texture("changelog").is_some());
}

#[test]
fn the_credit_names_people_then_the_clients_they_follow() {
    assert_eq!(credit_line(&["Sol".to_owned()], &[]), "By Sol");
    assert_eq!(
        credit_line(
            &["Sol".to_owned(), "Creyon".to_owned(), "Lumaya".to_owned()],
            &["JoF EJK".to_owned(), "OpenJK".to_owned()]
        ),
        "By Sol, Creyon and Lumaya, after JoF EJK and OpenJK"
    );
    assert_eq!(credit_line(&[], &["OpenJK".to_owned()]), "");
}

/// The real changelog's recent releases all lay out, each with its recap.
#[test]
fn the_real_changelog_lays_out() {
    let newest = crate::console::changelog::newest_release_digest().unwrap();
    assert!(!newest.lines.is_empty(), "{} has no recap", newest.title);
    let digests = crate::console::changelog::recent_digests("", RELEASES);
    assert_eq!(digests.len(), RELEASES);
    assert!(digests.iter().all(|digest| digest.title != "Unreleased"));
    let mut panel = Panel::default();
    panel.offer(entries(&sources(&digests, &[], &[])), None, true, "");
    for index in 0..RELEASES {
        panel.select(index);
        laid_out(&mut panel);
        assert!(!panel.laid.items.is_empty(), "{index}");
    }
}
