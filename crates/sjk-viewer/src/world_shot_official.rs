//! World shots of the SJK team's windows ([`crate::official`]) over duel6 under the SJK
//! UI: an announcement on the News panel (news, a release, a long one), the round list,
//! the claim window
//! and the creator's composer. Off-screen renders (no game window), ignored like the
//! other world shots: they need a GPU adapter and the installed game data named by
//! `JKA_GAME_DATA`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer official_windows_shot --
//! --ignored --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;
use crate::official::window::{Composer, Page};
use std::time::Duration;

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn official_windows_shot() {
    on_big_stack(|| {
        let size = [1920, 1080];
        let menu = menu::ClientMenu::new(true, String::new());
        let version = crate::alpha_notice_card::VERSION.to_string();
        let cvars = [
            ("ui_menuStyle", "sjk"),
            (crate::alpha_notice_card::CVAR, version.as_str()),
            // No First setup over the main menu, where the claim window opens.
            (crate::settings::quick::HIDE_CVAR, "1"),
        ];
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", size, Some(menu), &cvars) else {
            return;
        };
        let _ = frame(&mut gpu, 20);
        gpu.ui_epoch -= Duration::from_millis(2_000);
        for (kind, name) in [
            ("news", "official-news"),
            ("release", "official-release"),
            ("long", "official-long"),
        ] {
            let args = [kind.to_owned()];
            if let Some(console) = gpu.console.as_mut() {
                console.set_open(true);
            }
            gpu.debug_announce_command(&args).expect("a rehearsal");
            assert!(
                !gpu.console
                    .as_ref()
                    .is_some_and(console::ViewerConsole::is_open),
                "the command closes the console"
            );
            shoot(&mut gpu, 3, name);
            // Announcements are read on the News panel.
            assert!(gpu.news.is_open(), "{kind} shows");
            assert!(gpu.key_catcher_active(), "the cursor is free over {kind}");
            gpu.news_choice(Some(crate::news_panel::Choice::Close));
            assert!(!gpu.news.is_open(), "Close puts it away at once");
        }
        gpu.debug_reward_command(&["list".to_owned()])
            .expect("a rehearsal");
        shoot(&mut gpu, 3, "official-rounds");
        assert!(gpu.official.window.is_open(), "the round list shows");
        gpu.official_choice(Some(crate::official::window::Choice::Close));
        // On the main menu the reward waiting opens its claim window.
        shoot(&mut gpu, 3, "official-claim");
        assert!(
            matches!(gpu.official.window.page, Some(Page::Claim { .. })),
            "the claim window opens on the main menu"
        );
        gpu.official.show_for_shot(Page::Compose(Composer {
            release: true,
            version: "2026.1025.1".to_owned(),
            title: "SJK 2026.1025.1".to_owned(),
            text: "Announcements and contributor rewards are here.\n\n- Read the SJK team's news in SJK chat\n- Claim your rewards in the game".to_owned(),
            focus: crate::official::window::Field::Text,
            ..Composer::default()
        }));
        shoot(&mut gpu, 3, "official-compose");
        assert!(gpu.official.window.is_open(), "the composer shows");
    });
}
