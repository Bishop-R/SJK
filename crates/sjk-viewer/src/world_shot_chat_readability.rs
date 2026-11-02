//! World shots of the chat box's readability options over ffa3's sunlit sand (Creo's
//! report, 29/10/2026: the chat is hard to read over bright ground): as before, with
//! `cg_chatBoxBackdrop 50`, with `cg_chatBoxOutline 1`, and with both, at 1080p. Game
//! lines in several colours, a long wrapped one and two SJK chat lines (gold, with the
//! emblem and the tick). Off-screen renders (no game window), ignored like the other
//! world shots: they need a GPU adapter and the installed game data named by
//! `JKA_GAME_DATA`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test --release -p sjk-viewer chat_readability --
//! --ignored --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;

/// The camera: in ffa3's courtyard, looking down over the sand towards the walls.
const FROM: [f32; 3] = [1_050.0, 650.0, 230.0];
const YAW: f32 = 225.0;
const PITCH: f32 = -15.0;

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn ffa3_chat_readability() {
    on_big_stack(|| {
        let cvars = [("cg_chatbox", "600000"), ("cg_chatboxlines", "8")];
        let Some((mut gpu, _profile)) = open("maps/mp/ffa3.bsp", [1920, 1080], None, &cvars) else {
            return;
        };
        let _ = frame(&mut gpu, 1);
        aim(&mut gpu, FROM, YAW, PITCH);
        if let Some(console) = gpu.console.as_mut() {
            console.set_open(false);
        }
        let message = |id: u64, name: &str, text: &str| sjk_identity::ChatMessage {
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
            client: String::new(),
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
        for line in [
            "^7Kyle^7: ^2anyone up for a duel?",
            "^1Sol^7Vulpes^7: ^2HELLO there, the quick brown fox jumps over the lazy dog and keeps on running all the way",
            "^4Blue^7Fox^7: ^7can you read this over the sand?",
        ] {
            chat.receive(
                sjk_client::ServerEventKind::Chat,
                line.to_owned(),
                None,
                now,
            );
        }
        chat.sync_sjk(
            &state(vec![
                message(1, "^5Creyon", "the SJK chat in gold"),
                message(2, "^3Lumaya", "gg wp"),
            ]),
            |_| false,
            now,
        );
        chat.receive(
            sjk_client::ServerEventKind::Chat,
            "^6Mag^0enta^7: ^3yellow ^1red ^0black ^5cyan".to_owned(),
            None,
            now,
        );
        for (backdrop, outline, name) in [
            ("0", "0", "off"),
            ("50", "0", "backdrop50"),
            ("0", "1", "outline"),
            ("50", "1", "both"),
        ] {
            if let Some(console) = gpu.console.as_mut() {
                assert!(console.set_cvar(crate::chat::options::BACKDROP_CVAR, backdrop));
                assert!(console.set_cvar(crate::chat::options::OUTLINE_CVAR, outline));
            }
            let name = format!("ffa3-chat-readability-{name}-1080p");
            println!("{}", shoot(&mut gpu, 12, &name).display());
        }
    });
}
