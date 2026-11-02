//! World shots of the News panel over duel6 under the SJK UI: the newest release's recap
//! (as `whatsnew` previews it, with its settings to review), the panel on an older
//! release, and the rehearsed announcements (links, a switch, a picture's frame).
//! Off-screen renders (no game window), ignored like the other world shots: they need a
//! GPU adapter and the installed game data named by `JKA_GAME_DATA`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer whats_new_shots --
//! --ignored --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;
use crate::news_panel::{Choice, Key};

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn whats_new_shots() {
    on_big_stack(|| {
        let menu = menu::ClientMenu::new(true, String::new());
        let cvars = [
            ("ui_menuStyle", "sjk"),
            (crate::settings::quick::HIDE_CVAR, "1"),
        ];
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &cvars)
        else {
            return;
        };
        // The first frame reads the marker; a local build has no recap.
        let _ = frame(&mut gpu, 10);
        gpu.preview_whats_new();
        println!("{}", shoot(&mut gpu, 6, "duel6-news-preview").display());
        assert!(gpu.news.is_open(), "the panel shows");
        assert!(gpu.key_catcher_active(), "the cursor is free over it");
        gpu.news_choice(Some(Choice::Close));
        let _ = frame(&mut gpu, 2);
        assert!(!gpu.news.is_open(), "Close puts it away");

        // The panel on the older of the two newest releases.
        let release = crate::console::changelog::recent_digests("", 2)
            .pop()
            .map(|digest| Key::Release(digest.title));
        gpu.open_news(release, false);
        println!("{}", shoot(&mut gpu, 6, "duel6-news-release").display());
        assert!(gpu.news.is_open(), "the panel shows");
        gpu.news_choice(Some(Choice::Close));

        // The rehearsed announcements: links, a chip and a switch; a picture's frame.
        for (kind, name) in [
            ("long", "duel6-news-long"),
            ("picture", "duel6-news-picture"),
        ] {
            gpu.debug_announce_command(&[kind.to_owned()])
                .expect("a rehearsal");
            println!("{}", shoot(&mut gpu, 6, name).display());
            assert!(gpu.news.is_open(), "{kind} shows");
            gpu.news_choice(Some(Choice::Close));
        }
    });
}
