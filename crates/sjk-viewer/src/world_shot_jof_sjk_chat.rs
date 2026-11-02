//! World shots of SJK chat on JoF servers (`docs/mods.md`, "SJK chat on JoF servers")
//! over duel6: the notice card over the game menu, and the in-play chat box with an
//! ALL/SJK line, SJK-only lines (one from JoF EternalJK and one from the SJK Launcher,
//! each with its icon) and an All line,
//! then with the composer open on the SJK channel and its red note. Off-screen renders
//! (no game window), ignored like the other world shots: they need a GPU adapter and the
//! installed game data named by `JKA_GAME_DATA`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer jof_sjk_chat -- --ignored
//! --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;
use sjk_mod_jof::sjk_chat;

/// JoF tools' words while SJK chat is said in All chat, as the frame sets them.
fn words() -> crate::chat::server_copy::SaidInAll {
    crate::chat::server_copy::SaidInAll {
        note: sjk_chat::COMPOSER_NOTE,
        only: sjk_chat::ONLY_SJK,
        only_title: sjk_chat::ONLY_SJK_TITLE,
        only_why: &sjk_chat::ONLY_SJK_WHY,
    }
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_jof_sjk_chat_notice() {
    on_big_stack(|| {
        let Some((mut gpu, _profile)) = open(
            "maps/mp/duel6.bsp",
            [1920, 1080],
            None,
            &[("ui_menuStyle", "sjk")],
        ) else {
            return;
        };
        let _ = frame(&mut gpu, 4);
        if let Some(console) = gpu.console.as_mut() {
            console.set_open(false);
        }
        gpu.game_menu = true;
        gpu.mod_notice.card.show(crate::mod_notice_card::Notice {
            cvar: sjk_chat::NOTICE,
            kicker: sjk_chat::NOTICE_KICKER,
            title: sjk_chat::NOTICE_TITLE,
            paragraphs: &sjk_chat::NOTICE_PARAGRAPHS,
        });
        println!(
            "{}",
            shoot(&mut gpu, 8, "duel6-jof-sjk-chat-notice").display()
        );
        assert!(gpu.mod_notice.card.is_open(), "the card shows");
    });
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_jof_sjk_chat_feed() {
    on_big_stack(|| {
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", [1920, 1080], None, &[]) else {
            return;
        };
        let _ = frame(&mut gpu, 1);
        let shots = menu_backdrop::tour_for("Yavin Training Grounds").expect("duel6's tour");
        let shot = &shots[0];
        let (yaw, pitch) = look(shot.from, shot.at);
        aim(&mut gpu, shot.from, yaw, pitch);
        if let Some(console) = gpu.console.as_mut() {
            console.set_open(false);
        }
        let message = |id: u64, name: &str, text: &str, client: &str| sjk_identity::ChatMessage {
            reactions: Default::default(),
            id,
            at: 0,
            key_id: format!("{id:016x}"),
            name: name.to_owned(),
            verified: true,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            client: client.to_owned(),
            text: text.to_owned(),
            holocron: None,
            official: None,
        };
        let state = |messages: Vec<sjk_identity::ChatMessage>| sjk_identity::ChatState {
            messages: messages.into_iter().collect(),
            loaded: Some(1),
            ..sjk_identity::ChatState::default()
        };
        let now = std::time::Instant::now();
        let chat = &mut gpu.chat;
        chat.for_shot = true;
        chat.sync_sjk(&state(Vec::new()), |_| false, now);
        chat.receive(
            sjk_client::ServerEventKind::Chat,
            "^7Kyle^7: ^2anyone up for a duel?".to_owned(),
            None,
            now,
        );
        let both = message(1, "^1Sol^7Vulpes", "on my way, said in both chats", "sjk");
        let mut all = vec![both.clone()];
        chat.sync_sjk(&state(all.clone()), |_| false, now);
        // Its copy from the server, hidden and marking the line ALL/SJK.
        let copy = sjk_client::ServerEvent {
            kind: sjk_client::ServerEventKind::Chat,
            text: "^1Sol^7Vulpes^7: ^2^3SJK^2 on my way, said in both chats^9^1^1".to_owned(),
            sender: None,
            is_private: false,
        };
        assert!(chat.hold_server_copy(&copy, now));
        chat.follow_server_copies([&both], None, None, now);
        all.push(message(2, "^5Milamber", "hello from JoF EternalJK", "ejk"));
        all.push(message(
            3,
            "^2Jaden",
            "hello from the SJK Launcher",
            "launcher",
        ));
        all.push(message(
            4,
            "^3Lumaya",
            "SJK chat only, from the menus",
            "sjk",
        ));
        chat.sync_sjk(&state(all), |_| false, now);
        chat.set_said_in_all(Some(words()));
        println!(
            "{}",
            shoot(&mut gpu, 12, "duel6-jof-sjk-chat-feed").display()
        );
        gpu.chat.open_sjk();
        println!(
            "{}",
            shoot(&mut gpu, 12, "duel6-jof-sjk-chat-typing").display()
        );
    });
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_jof_sjk_chat_page_only() {
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
        let _ = frame(&mut gpu, 10);
        gpu.chat.for_shot = true;
        gpu.chat.set_said_in_all(Some(words()));
        let state = sjk_identity::ChatState {
            loaded: Some(1),
            ..sjk_identity::ChatState::default()
        };
        if let Some(console) = gpu.console.as_mut() {
            console.preview_sjk_chat(state, None);
            console.sjk_chat_only_for_shot("only for the SJK crew, sorry JoF");
        }
        println!(
            "{}",
            shoot(&mut gpu, 8, "duel6-jof-sjk-chat-page-only").display()
        );
        gpu.chat.for_shot = false;
        gpu.chat.set_said_in_all(None);
    });
}
