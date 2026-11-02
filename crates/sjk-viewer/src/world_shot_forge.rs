//! World shots of the Collection's Forge tab (`docs/sjk-ui.md`, Collection;
//! `docs/cards.md`, "The Forge") over the live duel6 in the SJK UI, on made-up holdings
//! and a made-up shop: the Shop (an offer waiting for its Confirm, one bought, one too
//! dear; while it reads the hub; after a purchase; a holocron for credits waiting for its
//! Confirm), Craft as a grid and as a list (a craft waiting for its Confirm, a search
//! with Affordable, nothing matching, a held blade card's copy crafting for less), Break down (a duplicate, a last copy, the card
//! asking before the last copy and before every duplicate, an older hub that breaks down
//! duplicates only, nothing held, after every duplicate went), the Forge while the hub
//! keeps it to staff and on a hub without an economy, the balances' hover card, and the
//! three sections in a 4:3 and a 4K window; since 29/10/2026 the collection marks
//! (`duel6_sjk_collection_marks`: Break down and the Shaders tab with favourites, a lock,
//! New ones and the worn blade). Off-screen renders (no game window), ignored
//! like the other world shots: they need a GPU adapter and the installed game data named
//! by `JKA_GAME_DATA` (and the hub's packs in `SJK_TEST_PACKS` for the blades).
//!
//! Run: `JKA_GAME_DATA=<GameData> SJK_TEST_PACKS=<hub>/assets/dist cargo test --release -p
//! sjk-viewer forge_shots -- --ignored --test-threads=1 --nocapture`; the pictures are in
//! `target/world-shots`.

use super::*;
use crate::console::collection_panel::forge::{self, Closed, ForgeData, Section};
use crate::player_menu::ReturnTarget;
use crate::profile_hub::Tab;

/// A made-up copy of `id`, held `count` times.
fn held(id: &str, count: u32) -> sjk_identity::Unlock {
    sjk_identity::Unlock {
        holo: 0,
        solar: 0,
        id: id.to_owned(),
        granted: 1_791_336_225,
        note: String::new(),
        count,
        medal: None,
        card: None,
    }
}

/// The made-up holdings: duplicates of a few shaders, single copies of others, a medal's
/// blade (never traded).
fn holdings() -> Vec<sjk_identity::Unlock> {
    vec![
        held("saber_sun", 1),
        held("saber_phoenix", 2),
        held("saber_tidal", 4),
        held("saber_circuit", 2),
        held("saber_aurora", 1),
        held("saber_frost", 1),
        held("saber_sakura", 3),
        sjk_identity::Unlock {
            medal: Some("bug_hunter".to_owned()),
            ..held("saber_glitch", 1)
        },
    ]
}

/// The Forge's made-up data: `shards` and 300 credits, the shop read with an hour left,
/// the hub's abilities as `abilities`.
fn data(shards: u64, abilities: bool) -> ForgeData {
    let mut data = forge::sample_data(shards, 300, 5 * 3_600 + 12 * 60, abilities);
    if let Some(shop) = data.shop.as_mut() {
        // The Uncommon was bought today.
        shop.day.offers[1].bought = true;
        shop.day.offers[1].owned = true;
    }
    data
}

fn sjk_cvars() -> [(&'static str, &'static str); 5] {
    [
        // No Discord status from a test.
        (crate::discord_presence::CVAR, "0"),
        ("ui_menuStyle", "sjk"),
        (crate::settings::quick::HIDE_CVAR, "1"),
        (crate::unlockables::SABER_SKIN_CVAR, "saber_frost"),
        ("name", "^1Sol^7Vulpes"),
    ]
}

/// The page ran out of room.
fn overflowed(gpu: &GpuState) -> bool {
    gpu.console
        .as_ref()
        .is_some_and(crate::console::ViewerConsole::profile_pages_overflowed)
}

/// The main menu over duel6 in `size`, the Collection open on the Forge's `section` with
/// `unlocks` and `data`.
fn open_forge(
    size: [u32; 2],
    unlocks: Vec<sjk_identity::Unlock>,
    data: ForgeData,
    section: Section,
) -> Option<GpuState> {
    super::saber_skins::mount_test_packs();
    let menu = menu::ClientMenu::new(true, String::new());
    let (mut gpu, profile) = open("maps/mp/duel6.bsp", size, Some(menu), &sjk_cvars())?;
    // The window's profile folder lives as long as the test.
    std::mem::forget(profile);
    gpu.looks.shot_owns_unlocks = true;
    let _ = frame(&mut gpu, 10);
    if let Some(console) = gpu.console.as_mut() {
        console.preview_profile(super::tests::profile_preview());
        console.preview_collection(Some(unlocks), 1.3);
        console.preview_forge(Some(data));
        console.forge_section_for_shot(section);
        console.open_profile_hub_page(Tab::Forge, ReturnTarget::MainMenu);
    }
    assert_eq!(gpu.profile_hub_tab(), Some(Tab::Forge));
    let _ = frame(&mut gpu, 30);
    Some(gpu)
}

/// Change what the Forge shows: the section, the holdings, the data, then the state a
/// shot wants ([`crate::console::ViewerConsole::forge_for_shot`]).
#[allow(clippy::too_many_arguments)]
fn set(
    gpu: &mut GpuState,
    section: Section,
    unlocks: Vec<sjk_identity::Unlock>,
    data: ForgeData,
    chosen: usize,
    (armed, ask, asked): (bool, bool, bool),
) {
    if let Some(console) = gpu.console.as_mut() {
        console.preview_collection(Some(unlocks), 1.3);
        console.preview_forge(Some(data));
        console.forge_section_for_shot(section);
    }
    // A frame lists the section's things before they are chosen.
    let _ = frame(gpu, 2);
    if let Some(console) = gpu.console.as_mut() {
        console.forge_for_shot(chosen, armed, ask, asked);
    }
}

/// A shader of the trade not in [`holdings`] with a blade card, and that card's id: the
/// packs' first, else a made-up blade card installed for it (the packs' pictures kept).
fn blade_card_shader() -> (&'static crate::unlockables::Unlockable, String) {
    let held = holdings();
    let lacks = |skin: &crate::unlockables::Unlockable| {
        crate::economy::in_trade(skin) && held.iter().all(|unlock| unlock.id != skin.id)
    };
    let faces = crate::cards::faces::current();
    if let Some((skin, id)) = faces.list.iter().find_map(|face| {
        let skin = crate::unlockables::find(face.blade.as_deref()?)?;
        lacks(skin).then(|| (skin, face.id.clone()))
    }) {
        return (skin, id);
    }
    let skin = crate::unlockables::blade_skins()
        .find(|skin| lacks(skin))
        .expect("a shader of the trade");
    let mut list = faces.list.clone();
    list.push(crate::cards::faces::Face {
        id: "sjk_blades_shot".to_owned(),
        name: skin.name.to_owned(),
        set: "sjk_blades".to_owned(),
        set_name: "SJK Blades".to_owned(),
        number: 1,
        kind: crate::cards::faces::Kind::Blade,
        rarity: &crate::cards::RARITIES[2],
        lore: "A made-up blade card for a world shot.".to_owned(),
        artist: "Sol".to_owned(),
        art: String::new(),
        grants: None,
        blade: Some(skin.id.to_owned()),
        scene: None,
        rank: None,
    });
    let real = faces.list.len();
    crate::cards::faces::install_for_shot(list, |index| index.min(real.saturating_sub(1)));
    (skin, "sjk_blades_shot".to_owned())
}

/// One revealed Holo copy of card `id`.
fn holo_copy(id: &str) -> sjk_identity::CardStack {
    sjk_identity::CardStack {
        card: Some(id.to_owned()),
        rarity: "rare".to_owned(),
        finish: "holo".to_owned(),
        edition: "alpha".to_owned(),
        count: 1,
        first: 1_791_641_100,
        last: 1_791_641_100,
    }
}

/// A trade's made-up outcome.
fn outcome(
    trade: sjk_identity::Trade,
    id: &str,
    amount: u64,
    copies: u32,
) -> sjk_identity::TradeOutcome {
    sjk_identity::TradeOutcome {
        serial: 2,
        trade,
        id: id.to_owned(),
        amount,
        copies,
        message: String::new(),
    }
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_forge_shots() {
    on_big_stack(|| {
        let Some(mut gpu) = open_forge([1920, 1080], holdings(), data(300, true), Section::Shop)
        else {
            return;
        };
        let mut moments = Vec::new();
        let mut keep = |gpu: &mut GpuState, frames: u32, name: &str| {
            let path = shoot(gpu, frames, &format!("duel6-forge-{name}"));
            assert!(!overflowed(gpu), "{name}");
            println!("{}", path.display());
            moments.push(image::open(path).expect("the shot").to_rgba8());
        };
        let set_grid = |gpu: &mut GpuState, grid: bool| {
            if let Some(console) = gpu.console.as_mut() {
                let _ = console.set_cvar(
                    crate::console::collection_panel::SHADER_VIEW_CVAR,
                    if grid { "0" } else { "1" },
                );
            }
        };
        // The Shop: the Common waiting for its Confirm, the Uncommon bought, the Mythical
        // too dear.
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            data(300, true),
            0,
            (true, false, false),
        );
        keep(&mut gpu, 4, "shop");
        let mut reading = data(300, true);
        reading.shop = None;
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            reading,
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "shop-reading");
        let mut bought = data(225, true);
        bought.trade = Some(outcome(sjk_identity::Trade::Buy, "saber_sakura", 75, 0));
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            bought,
            1,
            (false, false, true),
        );
        keep(&mut gpu, 2, "shop-bought");
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            data(300, true),
            4,
            (true, false, false),
        );
        keep(&mut gpu, 2, "shop-holocron-confirm");
        // Craft, as a grid and as a list.
        set_grid(&mut gpu, true);
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            data(320, true),
            2,
            (false, false, false),
        );
        keep(&mut gpu, 4, "craft-grid");
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            data(320, true),
            2,
            (true, false, false),
        );
        keep(&mut gpu, 2, "craft-confirm");
        set_grid(&mut gpu, false);
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            data(320, true),
            7,
            (false, false, false),
        );
        keep(&mut gpu, 4, "craft-list");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_search_for_shot("blade", true);
        }
        keep(&mut gpu, 2, "craft-affordable");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_search_for_shot("zzz", false);
        }
        keep(&mut gpu, 2, "craft-nothing");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_search_for_shot("", false);
        }
        let mut crafted = data(20, true);
        crafted.trade = Some(outcome(sjk_identity::Trade::Craft, "saber_echo", 300, 0));
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            crafted,
            0,
            (false, false, true),
        );
        keep(&mut gpu, 2, "craft-short");
        // A Holo copy of a shader's blade card held: it crafts for half, and says so.
        let blade = blade_card_shader();
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(vec![holo_copy(&blade.1)], 300, 320);
            console.forge_search_for_shot(blade.0.name, false);
        }
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            data(320, true),
            0,
            (false, false, false),
        );
        if let Some(console) = gpu.console.as_mut() {
            console.forge_want_for_shot(blade.0.id);
        }
        keep(&mut gpu, 4, "craft-blade-card");
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(Vec::new(), 300, 320);
            console.forge_search_for_shot("", false);
        }
        // Break down: a duplicate, a last copy (worn), the cards asking.
        set_grid(&mut gpu, true);
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            1,
            (false, false, false),
        );
        keep(&mut gpu, 4, "break-grid");
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            1,
            (true, false, false),
        );
        keep(&mut gpu, 2, "break-confirm");
        let last = 5;
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            last,
            (false, false, false),
        );
        keep(&mut gpu, 2, "break-last");
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            last,
            (false, true, false),
        );
        keep(&mut gpu, 4, "break-ask-last");
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            0,
            (false, true, false),
        );
        keep(&mut gpu, 4, "break-ask-all");
        let mut gone = data(355, true);
        gone.trade = Some(outcome(sjk_identity::Trade::ShatterAll, "", 195, 6));
        let after: Vec<_> = holdings()
            .into_iter()
            .map(|unlock| sjk_identity::Unlock { count: 1, ..unlock })
            .collect();
        set(
            &mut gpu,
            Section::Break,
            after,
            gone,
            0,
            (false, false, true),
        );
        keep(&mut gpu, 2, "break-all-done");
        set_grid(&mut gpu, false);
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, true),
            2,
            (false, false, false),
        );
        keep(&mut gpu, 4, "break-list");
        // An older hub: duplicates only, no Break down all duplicates.
        set_grid(&mut gpu, true);
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(160, false),
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "break-old-hub");
        set(
            &mut gpu,
            Section::Break,
            vec![held("saber_aurora", 1)],
            data(0, false),
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "break-no-duplicates");
        set(
            &mut gpu,
            Section::Break,
            Vec::new(),
            data(0, true),
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "break-empty");
        // Kept to staff by the hub, and a hub without an economy.
        let staff = ForgeData {
            can_trade: false,
            why_not: Some("The Forge opens with holocrons: soon.".to_owned()),
            shop: None,
            ..data(160, true)
        };
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            staff.clone(),
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "staff-only");
        set(
            &mut gpu,
            Section::Craft,
            holdings(),
            staff,
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "staff-only-craft");
        let none = ForgeData {
            closed: Closed::NoEconomy,
            ..ForgeData::default()
        };
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            none,
            0,
            (false, false, false),
        );
        keep(&mut gpu, 2, "no-economy");
        // The balances' hover card.
        set(
            &mut gpu,
            Section::Shop,
            holdings(),
            data(300, true),
            0,
            (false, false, false),
        );
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(Some(usize::from(
                crate::console::collection_panel::forge::SHARDS_TOKEN,
            )));
        }
        keep(&mut gpu, 2, "hover-shards");
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(None);
        }
        println!(
            "{}",
            sheet(&moments, 4, 640, "duel6-forge-moments").display()
        );
    });
}

/// The three sections in a 4:3 and a 4K window.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_forge_windows() {
    on_big_stack(|| {
        for (size, name) in [([1440, 1080], "4x3"), ([3840, 2160], "4k")] {
            let Some(mut gpu) = open_forge(size, holdings(), data(300, true), Section::Shop) else {
                return;
            };
            if let Some(console) = gpu.console.as_mut() {
                let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "0");
            }
            for (section, part, chosen) in [
                (Section::Shop, "shop", 0),
                (Section::Craft, "craft", 2),
                (Section::Break, "break", 1),
            ] {
                set(
                    &mut gpu,
                    section,
                    holdings(),
                    data(300, true),
                    chosen,
                    (false, false, false),
                );
                let path = shoot(&mut gpu, 4, &format!("duel6-forge-{part}-{name}"));
                assert!(!overflowed(&gpu), "{part} {name}");
                println!("{}", path.display());
            }
            set(
                &mut gpu,
                Section::Break,
                holdings(),
                data(300, true),
                6,
                (false, true, false),
            );
            let path = shoot(&mut gpu, 4, &format!("duel6-forge-break-ask-last-{name}"));
            println!("{}", path.display());
        }
    });
}

/// Collection marks (29/10/2026; Atrosex's and Kushina's ideas, extended by Sol): Break
/// down as a grid and as a list with the favourites first (Phoenix and Aurora), a locked
/// duplicate (Tidal), New ones (Sakura, Circuit) and the worn blade (Frost); the locked one
/// chosen, its button saying Locked; the Favourites chip; the last copy of the worn Frost
/// asking, which says the player wears it; then the Shaders tab's wall and list with the
/// same marks, the New count and Mark all seen, the card's star and padlock, and a hover
/// card saying what each mark means.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_collection_marks() {
    on_big_stack(|| {
        let Some(mut gpu) = open_forge([1920, 1080], holdings(), data(300, true), Section::Break)
        else {
            return;
        };
        let marks = crate::shader_marks::Marks::shown(
            &["saber_phoenix", "saber_aurora"],
            &["saber_tidal"],
            &["saber_sakura", "saber_circuit"],
            Some("saber_frost"),
        );
        if let Some(console) = gpu.console.as_mut() {
            console.collection_marks_for_shot(Some(marks));
            let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "0");
        }
        let keep = |gpu: &mut GpuState, frames: u32, name: &str| {
            let path = shoot(gpu, frames, &format!("duel6-marks-{name}"));
            assert!(!overflowed(gpu), "{name}");
            println!("{}", path.display());
        };
        set(
            &mut gpu,
            Section::Break,
            holdings(),
            data(300, true),
            0,
            (false, false, false),
        );
        keep(&mut gpu, 4, "forge-break-grid");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_want_for_shot("saber_tidal");
        }
        keep(&mut gpu, 4, "forge-break-locked");
        if let Some(console) = gpu.console.as_mut() {
            let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "1");
            console.forge_want_for_shot("saber_sakura");
        }
        keep(&mut gpu, 4, "forge-break-list");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_favourites_for_shot(true);
        }
        keep(&mut gpu, 4, "forge-break-favourites");
        if let Some(console) = gpu.console.as_mut() {
            console.forge_favourites_for_shot(false);
            let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "0");
        }
        let _ = frame(&mut gpu, 2);
        if let Some(console) = gpu.console.as_mut() {
            console.forge_ask_last_for_shot("saber_frost");
        }
        keep(&mut gpu, 4, "forge-ask-last-worn");
        // The Shaders tab: the wall, then the list, a favourite chosen.
        if let Some(console) = gpu.console.as_mut() {
            console.preview_forge(None);
            console.collection_trade_for_shot(true);
            console.open_profile_hub_page(Tab::Shaders, ReturnTarget::MainMenu);
        }
        gpu.profile_hub_show_for_shot(Tab::Shaders);
        // A frame lists the rows (the favourites first) before the first is chosen.
        let _ = frame(&mut gpu, 2);
        if let Some(console) = gpu.console.as_mut() {
            console.collection_shader_for_shot(1);
        }
        keep(&mut gpu, 30, "shaders-wall");
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(Some(1));
        }
        keep(&mut gpu, 30, "shaders-hover");
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(None);
            let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "1");
        }
        keep(&mut gpu, 30, "shaders-list");
    });
}

/// #117 (Nib: the Forge flickered on the last lines of Craft): its grid with every swatch
/// drawn (the menu pictures on), on each line from the Rare ones to the last, through a few
/// seconds of their animation, never fills the page's draw commands. With 2 048 the
/// Legendary lines took up to about 2 100 on some frames, which dropped the last ones.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_forge_drawn_swatches_fit() {
    on_big_stack(|| {
        let Some(mut gpu) = open_forge([1920, 1080], holdings(), data(320, true), Section::Craft)
        else {
            return;
        };
        if let Some(console) = gpu.console.as_mut() {
            let _ = console.set_cvar(crate::console::collection_panel::SHADER_VIEW_CVAR, "0");
            let _ = console.set_cvar(crate::menu_pictures::CVAR, "1");
        }
        for chosen in (60..=160).step_by(10) {
            set(
                &mut gpu,
                Section::Craft,
                holdings(),
                data(320, true),
                chosen,
                (false, false, false),
            );
            for step in 0..24 {
                if let Some(console) = gpu.console.as_mut() {
                    console.preview_collection(Some(holdings()), step as f32 * 0.137);
                }
                let _ = frame(&mut gpu, 1);
                assert!(!overflowed(&gpu), "shader {chosen} at step {step}");
            }
        }
    });
}
