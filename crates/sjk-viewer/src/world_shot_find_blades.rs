//! World shots of grand finds in the game's chat (`docs/holocrons.md`, "Chat lines"):
//! over duel6, a holocron drop and three finds (a Mythical blade from a Legendary Kyber
//! Crystal, the Phoenix blade from a Rare one, a Holo card from a Mythical holocron),
//! each starting with its holocron's or crystal's picture, the prize in its own colour
//! and a blade small after the picture; then with the composer open and the pointer on
//! a blade, its large card above the line. Off-screen renders (no game window), ignored
//! like the other world shots: they need a GPU adapter and the installed game data named
//! by `JKA_GAME_DATA`.
//!
//! The blades come from the hub's packs in `SJK_TEST_PACKS` (grey without them).
//!
//! Run: `JKA_GAME_DATA=<GameData> SJK_TEST_PACKS=<packs> cargo test -p sjk-viewer find_blades -- --ignored
//! --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_find_blades_in_chat() {
    on_big_stack(|| {
        super::saber_skins::mount_test_packs();
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
        let now = std::time::Instant::now();
        let mark = |tier: &str, find: Option<sjk_identity::FindMark>| sjk_identity::DropMark {
            tier: tier.to_owned(),
            own: false,
            find,
        };
        let line = |id: u64, name: &str, holocron| sjk_identity::ChatMessage {
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
            client: "sjk".to_owned(),
            text: String::new(),
            holocron: Some(holocron),
            official: None,
            channel: Default::default(),
        };
        let blade = |skin: &crate::unlockables::Unlockable| sjk_identity::FindMark {
            shader: true,
            name: skin.name.to_owned(),
            rarity: skin.tier.label().to_lowercase(),
            finish: String::new(),
            card: String::new(),
            skin: skin.id.to_owned(),
        };
        let mythical = crate::unlockables::blade_skins_by_tier()
            .iter()
            .copied()
            .find(|skin| skin.tier == crate::unlockables::Rarity::Mythical)
            .expect("a Mythical blade");
        let phoenix = crate::unlockables::blade_skin("saber_phoenix").expect("Phoenix");
        let card = sjk_identity::FindMark {
            shader: false,
            name: String::new(),
            rarity: "legendary".to_owned(),
            finish: "holo".to_owned(),
            card: String::new(),
            skin: String::new(),
        };
        let messages = vec![
            line(1, "^2Jaden", mark("legendary", None)),
            line(
                2,
                "^1Warrior ^7SmoKeY",
                mark("kyber_legendary", Some(blade(mythical))),
            ),
            line(3, "^5Milamber", mark("mythical", Some(card))),
            line(4, "^1Sol^7Vulpes", mark("kyber_rare", Some(blade(phoenix)))),
        ];
        let state = sjk_identity::ChatState {
            messages: messages.into_iter().collect(),
            loaded: Some(1),
            ..sjk_identity::ChatState::default()
        };
        let chat = &mut gpu.chat;
        chat.for_shot = true;
        chat.sync_sjk(
            &sjk_identity::ChatState {
                loaded: Some(1),
                ..Default::default()
            },
            |_| false,
            now,
        );
        chat.sync_sjk(&state, |_| false, now);
        println!(
            "{}",
            shoot(&mut gpu, 12, "duel6-find-blades-feed").display()
        );
        gpu.chat.open_sjk();
        let _ = frame(&mut gpu, 2);
        let rect = gpu.chat.find_blade_for_shot().expect("a blade to rest on");
        gpu.chat
            .pointer(sjk_ui::InputEvent::PointerMove(sjk_ui::Vec2::new(
                rect.x + rect.width * 0.5,
                rect.y + rect.height * 0.5,
            )));
        println!(
            "{}",
            shoot(&mut gpu, 12, "duel6-find-blades-hover").display()
        );
        gpu.chat.for_shot = false;
    });
}
