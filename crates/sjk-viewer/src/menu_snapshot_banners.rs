//! Existing CPU snapshot renderer applied to real profile and sender-card components.
use super::*;
use crate::game_font::SjkFonts;
use crate::menu::sjk::{Frame, color, text};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use sjk_ui::{FontWeight, TextAlign};

#[test]
#[ignore = "writes an offline component screenshot using the bundled UI fonts"]
fn profile_banner_cards_snapshot() {
    let viewport = [1920.0, 1080.0];
    let frame = Frame::new(viewport);
    let display = crate::text::load_family(&crate::text::DISPLAY, 1.0, None).unwrap();
    let body = crate::text::load_family(&crate::text::BODY, 1.0, None).unwrap();
    let mut canvas = MenuCanvas::with_draw_capacity(1024);
    canvas.begin_transparent(viewport);
    text(
        &mut canvas,
        TextFamily::Display,
        format_args!("Profile banners"),
        frame.rect(96.0, 64.0, 1728.0, 64.0),
        48.0,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    text(
        &mut canvas,
        TextFamily::Body,
        format_args!("One identity, the same cover on your profile and player cards"),
        frame.rect(96.0, 146.0, 1728.0, 40.0),
        24.0,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::Start,
    );
    crate::profile_card::banner::draw(
        &mut canvas,
        frame.rect(96.0, 220.0, 1728.0, 350.0),
        "banner_hyperspace",
    );
    let summary = crate::profile_card::Summary {
        name: "^3Banner Explorer".to_owned(),
        key_id: "0123456789abcdef".to_owned(),
        banner: "banner_hyperspace",
        hub: crate::profile_card::HubState::Online,
        verified: true,
        ..Default::default()
    };
    crate::profile_card::draw(
        &mut canvas,
        &frame,
        &crate::profile_card::Card {
            name: &summary.name,
            detail: crate::profile_card::Detail::Line("Your profile card"),
            summary: &summary,
            lit: true,
        },
        100,
    );
    let person = crate::sender_card::Person {
        name: "^3Banner Explorer".to_owned(),
        key_id: Some(summary.key_id.clone()),
        avatar: Some(String::new()),
        banner: summary.banner,
        verified: true,
        ..Default::default()
    };
    crate::sender_card::draw(
        &mut canvas,
        &crate::sender_card::Card {
            person: &person,
            muted: false,
            measure: None,
        },
        [1120.0, 670.0],
        1.4,
        crate::sender_card::Tokens {
            card: 101,
            mute: 102,
            ban: 103,
            safety: 104,
            report: 105,
            whisper: None,
            trade: 106,
            map_block: 107,
            staff_block: 108,
        },
    );
    assert!(!canvas.overflowed());
    let (mut display_text, mut body_text) = (Vec::new(), Vec::new());
    canvas.append_text_families(
        SjkFonts {
            display: (&mut display_text, &display.font),
            body: (&mut body_text, &body.font),
            code: None,
        },
        viewport,
        crate::text::TextStyle::NEUTRAL,
    );
    let backdrop = RgbaImage::from_pixel(1920, 1080, Rgba([8, 13, 24, 255]));
    save_sjk_shot(
        "profile-banner-cards",
        viewport,
        &backdrop,
        canvas.draw_list(),
        &HashMap::new(),
        SjkShotFonts {
            display: (&display_text, &display.image),
            body: (&body_text, &body.image),
        },
    );
}
