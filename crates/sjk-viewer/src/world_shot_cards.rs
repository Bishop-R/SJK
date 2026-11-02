//! World shots of opening holocrons and of the Collection's Cards tab
//! ([`crate::console::holocrons_panel::opening`], `collection_cards.rs`) over the live duel6
//! in the SJK UI: the Holocrons tab with its Open buttons and the credits; the ceremony at
//! each moment of a card's entrance (the burst, the card turning in, the credits counting,
//! the words) in each finish; the wait for the hub and a refusal; then the Cards tab with
//! every kind of sealed card, a Solar one chosen, and with none; and the loot table over
//! the Holocrons tab. Made-up holdings; they
//! are off-screen renders (no game window), ignored like the other world shots: they need
//! a GPU adapter and the installed game data named by `JKA_GAME_DATA`.
//!
//! Then the first revealed card's face (`duel6_sjk_card_faces`), from the `sjk_cards` pack
//! in the folder `SJK_TEST_PACKS` names (the hub's `assets/dist`): up close on the Cards
//! tab in each finish, and coming out of a holocron. And the Cards tab as a binder
//! (`duel6_sjk_card_binder`, 19/10/2026) with the same pack: every card, found or greyed,
//! sealed copies, hover cards and the filter, in 1080p and 4:3, and since 20/10/2026 in
//! 4K, with a card looked at closely (the Toxiee card, a short lore and a long one).
//! Then the finishes in motion (`duel6_sjk_card_finish_motion`: Solar and Holo up close
//! and on their tile, and Solar out of a holocron, at three moments each) and a binder of
//! many cards in several sets, scrolled (`duel6_sjk_card_binder_many`, a made-up catalogue over the pack's pictures).
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer cards_shots -- --ignored
//! --test-threads=1 --nocapture`; the pictures are in `target/world-shots`.

use super::*;
use crate::cards::binder::{Filter, Show};
use crate::cards::{FINISHES, RARITIES};
use crate::console::holocrons_panel::Data;
use crate::console::holocrons_panel::opening::{Prize, Shown};
use crate::profile_hub::Tab;

/// The Holocrons tab's made-up holdings with holocrons to open and credits.
fn holdings() -> Data {
    Data {
        sealed: [vec![11, 12, 15], vec![13], Vec::new(), vec![14]],
        credits: Some(1_340),
        ..super::holocrons_tab::holdings()
    }
}

/// One holocron of tier `tier` opened: a card of `rarity` in `finish`.
fn shown(tier: usize, rarity: usize, finish: usize, credits: u64) -> Shown {
    Shown {
        holocron: 1,
        tier,
        rarity: &RARITIES[rarity],
        finish: &FINISHES[finish],
        edition: "Alpha Edition".to_owned(),
        credits,
        shards: 0,
        card: None,
        bonus: None,
    }
}

/// Every kind of sealed card: each rarity in each finish.
fn every_kind() -> Vec<sjk_identity::CardStack> {
    let mut list = Vec::new();
    for finish in &FINISHES {
        for rarity in &RARITIES {
            list.push(sjk_identity::CardStack {
                card: None,
                rarity: rarity.id.to_owned(),
                finish: finish.id.to_owned(),
                edition: "alpha".to_owned(),
                count: (list.len() as u32 % 4) + 1,
                first: 1_791_641_100,
                last: 1_791_641_100,
            });
        }
    }
    list
}

fn sjk_cvars() -> [(&'static str, &'static str); 4] {
    [
        // No Discord status from a test.
        (crate::discord_presence::CVAR, "0"),
        ("ui_menuStyle", "sjk"),
        (crate::settings::quick::HIDE_CVAR, "1"),
        ("name", "^1Sol^7Vulpes"),
    ]
}

/// The main menu over duel6 in `size`, the Collection open on its Holocrons tab (the
/// loot table shown by [`duel6_sjk_loot_table`]).
fn open_holocrons(size: [u32; 2]) -> Option<GpuState> {
    let menu = menu::ClientMenu::new(true, String::new());
    let (mut gpu, profile) = open("maps/mp/duel6.bsp", size, Some(menu), &sjk_cvars())?;
    // The window's profile folder lives as long as the test.
    std::mem::forget(profile);
    let _ = frame(&mut gpu, 10);
    gpu.holocron_stage_seconds = Some(2.4);
    if let Some(console) = gpu.console.as_mut() {
        console.preview_holocrons(holdings(), 0);
    }
    gpu.open_profile_hub(Tab::Holocrons);
    assert_eq!(gpu.profile_hub_tab(), Some(Tab::Holocrons));
    let _ = frame(&mut gpu, 150);
    Some(gpu)
}

fn overflowed(gpu: &GpuState) -> bool {
    gpu.console.as_ref().is_some_and(|console| {
        console.holocrons_page_overflowed() || console.profile_pages_overflowed()
    })
}

/// Play `cards`' ceremony drawn `elapsed` seconds into the first card's entrance.
fn ceremony(gpu: &mut GpuState, cards: Vec<Shown>, elapsed: f32) {
    if let Some(console) = gpu.console.as_mut() {
        let prizes = cards.into_iter().map(Prize::Card).collect();
        console.preview_opening(Some(prizes), elapsed, 3.1);
    }
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_opening() {
    on_big_stack(|| {
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        println!("{}", shoot(&mut gpu, 2, "duel6-opening-page").display());
        assert!(!overflowed(&gpu));
        // A player while the hub keeps opening to staff: a line in place of the buttons.
        if let Some(console) = gpu.console.as_mut() {
            console.preview_holocrons(
                Data {
                    can_open: false,
                    ..holdings()
                },
                0,
            );
        }
        let _ = frame(&mut gpu, 4);
        println!("{}", shoot(&mut gpu, 2, "duel6-opening-closed").display());
        assert!(!overflowed(&gpu));
        if let Some(console) = gpu.console.as_mut() {
            console.preview_holocrons(holdings(), 0);
        }
        // A Holo card's entrance, moment by moment.
        let mut moments = Vec::new();
        for (elapsed, name) in [
            (0.3, "burst"),
            (0.8, "turning"),
            (1.5, "counting"),
            (3.0, "words"),
        ] {
            ceremony(
                &mut gpu,
                vec![shown(1, 2, 1, 42), shown(0, 0, 0, 14)],
                elapsed,
            );
            let path = shoot(&mut gpu, 2, &format!("duel6-opening-holo-{name}"));
            assert!(!overflowed(&gpu), "{name}");
            println!("{}", path.display());
            moments.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&moments, 2, 960, "duel6-opening-moments").display()
        );
        // Each finish, standing still.
        let mut finishes = Vec::new();
        for (tier, rarity, finish, credits) in [(0, 0, 0, 14), (2, 3, 1, 96), (3, 1, 2, 311)] {
            ceremony(&mut gpu, vec![shown(tier, rarity, finish, credits)], 3.0);
            let path = shoot(
                &mut gpu,
                2,
                &format!("duel6-opening-{}", FINISHES[finish].id),
            );
            assert!(!overflowed(&gpu));
            println!("{}", path.display());
            finishes.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&finishes, 3, 640, "duel6-opening-finishes").display()
        );
        // The wait for the hub, then a refusal.
        if let Some(console) = gpu.console.as_mut() {
            console.preview_opening(None, 0.0, 0.0);
            console.preview_opening_state(None);
        }
        println!("{}", shoot(&mut gpu, 2, "duel6-opening-waiting").display());
        if let Some(console) = gpu.console.as_mut() {
            console.preview_opening_state(Some("That holocron was already opened."));
        }
        println!("{}", shoot(&mut gpu, 2, "duel6-opening-refused").display());
        assert!(!overflowed(&gpu));
    });
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_cards_tab() {
    on_big_stack(|| {
        for (size, name) in [([1920, 1080], "1080p"), ([1440, 1080], "4x3")] {
            let menu = menu::ClientMenu::new(true, String::new());
            let Some((mut gpu, _profile)) =
                open("maps/mp/duel6.bsp", size, Some(menu), &sjk_cvars())
            else {
                return;
            };
            let _ = frame(&mut gpu, 10);
            if let Some(console) = gpu.console.as_mut() {
                console.preview_cards(every_kind(), 1_340, 85);
                console.preview_collection(Some(Vec::new()), 3.1);
            }
            gpu.open_profile_hub(Tab::Cards);
            assert_eq!(gpu.profile_hub_tab(), Some(Tab::Cards));
            let _ = frame(&mut gpu, 120);
            // The second sealed tile (Legendary).
            if let Some(console) = gpu.console.as_mut() {
                console.collection_card_for_shot(1);
            }
            println!(
                "{}",
                shoot(&mut gpu, 4, &format!("duel6-cards-{name}")).display()
            );
            assert!(!overflowed(&gpu), "{name}");
        }
        let menu = menu::ClientMenu::new(true, String::new());
        let Some((mut gpu, _profile)) =
            open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &sjk_cvars())
        else {
            return;
        };
        let _ = frame(&mut gpu, 10);
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(Vec::new(), 0, 0);
        }
        gpu.open_profile_hub(Tab::Cards);
        let _ = frame(&mut gpu, 120);
        println!("{}", shoot(&mut gpu, 2, "duel6-cards-none").display());
    });
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_loot_table() {
    on_big_stack(|| {
        for (size, name) in [([1920, 1080], "1080p"), ([1440, 1080], "4x3")] {
            let Some(mut gpu) = open_holocrons(size) else {
                return;
            };
            if size[0] == 1920 {
                // The tab with its Loot table button.
                println!("{}", shoot(&mut gpu, 2, "duel6-loot-button").display());
            }
            if let Some(console) = gpu.console.as_mut() {
                console.preview_loot(true);
            }
            println!(
                "{}",
                shoot(&mut gpu, 2, &format!("duel6-loot-table-{name}")).display()
            );
            assert!(!overflowed(&gpu), "{name}");
        }
    });
}

/// The first card, revealed, in `finish`, beside sealed ones.
fn with_face(finish: &str) -> Vec<sjk_identity::CardStack> {
    let mut list = vec![sjk_identity::CardStack {
        card: Some("sjk_alpha_010".to_owned()),
        rarity: "legendary".to_owned(),
        finish: finish.to_owned(),
        edition: "alpha".to_owned(),
        count: 1,
        first: 1_791_830_000,
        last: 1_791_830_000,
    }];
    list.extend(every_kind().into_iter().take(5));
    list
}

/// Where the binder (`crate::cards::binder`) puts card `id` among `list`'s tiles, with
/// the packs' faces.
fn binder_index(list: &[sjk_identity::CardStack], id: &str) -> usize {
    let faces = crate::cards::faces::current();
    crate::cards::binder::tiles(&crate::cards::stacks(list), &faces)
        .iter()
        .position(|tile| tile.face(&faces).is_some_and(|face| face.id == id))
        .unwrap_or(0)
}

/// Copies of SJK Alpha's card `number` in `finish`, `count` of them.
fn alpha(number: u16, rarity: &str, finish: &str, count: u32) -> sjk_identity::CardStack {
    sjk_identity::CardStack {
        card: Some(format!("sjk_alpha_{number:03}")),
        rarity: rarity.to_owned(),
        finish: finish.to_owned(),
        edition: "alpha".to_owned(),
        count,
        first: 1_791_830_000 + i64::from(number) * 3_600,
        last: 1_791_830_000,
    }
}

/// Sealed copies of `rarity` in `finish`.
fn sealed(rarity: &str, finish: &str, count: u32) -> sjk_identity::CardStack {
    sjk_identity::CardStack {
        card: None,
        rarity: rarity.to_owned(),
        finish: finish.to_owned(),
        edition: "alpha".to_owned(),
        count,
        first: 1_791_641_100,
        last: 1_791_641_100,
    }
}

/// A binder partly filled: sealed copies of three rarities (a Legendary one the packs
/// can reveal), five cards of SJK Alpha found, two of them in several finishes, the
/// rest missing.
fn binder_holdings() -> Vec<sjk_identity::CardStack> {
    vec![
        sealed("legendary", "normal", 1),
        sealed("rare", "normal", 3),
        sealed("rare", "holo", 1),
        sealed("common", "normal", 2),
        alpha(0, "mythical", "normal", 1),
        alpha(1, "mythical", "holo", 1),
        alpha(1, "mythical", "normal", 1),
        alpha(4, "legendary", "normal", 1),
        alpha(7, "legendary", "holo", 2),
        alpha(10, "legendary", "solar", 1),
        alpha(10, "legendary", "normal", 2),
    ]
}

/// The main menu over duel6 in `size`, the Collection open on its Cards tab showing
/// [`binder_holdings`], the hub letting the player open (so Reveal shows).
fn open_binder(size: [u32; 2]) -> Option<GpuState> {
    let menu = menu::ClientMenu::new(true, String::new());
    let (mut gpu, profile) = open("maps/mp/duel6.bsp", size, Some(menu), &sjk_cvars())?;
    // The window's profile folder lives as long as the test.
    std::mem::forget(profile);
    let _ = frame(&mut gpu, 10);
    let mut preview = super::tests::profile_preview();
    preview.snapshot.holocrons = Some(sjk_identity::HolocronProgress {
        progress_secs: 0,
        every_secs: 1_800,
        today: 0,
        daily_cap: 8,
        resets_at: None,
        server_time: 1_791_900_000,
        can_open: true,
        read_at: std::time::Instant::now(),
    });
    if let Some(console) = gpu.console.as_mut() {
        console.preview_profile(preview);
        console.preview_cards(binder_holdings(), 1_340, 85);
        console.preview_collection(Some(Vec::new()), 3.1);
    }
    gpu.open_profile_hub(Tab::Cards);
    assert_eq!(gpu.profile_hub_tab(), Some(Tab::Cards));
    let _ = frame(&mut gpu, 60);
    Some(gpu)
}

/// The binder (19/10/2026) with the hub's card pack mounted, in 1080p and 4:3: every
/// card of the packs, found or greyed, sealed copies first, Reveal, a hover card on a
/// card held in two finishes and on a missing one, and the filter narrowing it.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_card_binder() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let list = binder_holdings();
        let toxiee = binder_index(&list, "sjk_alpha_010");
        let missing = binder_index(&list, "sjk_alpha_005");
        let filters = [
            ("all", Filter::default()),
            (
                "owned",
                Filter {
                    show: Show::Owned,
                    ..Filter::default()
                },
            ),
            (
                "legendary-missing",
                Filter {
                    show: Show::Missing,
                    rarity: crate::cards::Rarity::from_id("legendary"),
                    ..Filter::default()
                },
            ),
            (
                "holo",
                Filter {
                    finish: crate::cards::Finish::from_id("holo"),
                    ..Filter::default()
                },
            ),
        ];
        for (size, name) in [
            ([1920, 1080], "1080p"),
            ([1440, 1080], "4x3"),
            ([3840, 2160], "4k"),
        ] {
            let Some(mut gpu) = open_binder(size) else {
                return;
            };
            for (filter_name, filter) in &filters {
                // At 4K the whole binder and the looks closer only.
                if name == "4k" && *filter_name != "all" {
                    continue;
                }
                // All: the Toxiee card, held in two finishes, chosen with its hover card;
                // the Legendary missing ones: a hover card on one; the rest their first
                // tile chosen.
                let (chosen, hover) = match *filter_name {
                    "all" => (toxiee, Some(toxiee)),
                    "legendary-missing" => (1, Some(2)),
                    _ => (0, None),
                };
                if let Some(console) = gpu.console.as_mut() {
                    console.collection_card_filter_for_shot(*filter);
                    console.collection_card_for_shot(chosen);
                }
                let _ = frame(&mut gpu, 4);
                if let Some(console) = gpu.console.as_mut() {
                    console.collection_hover_for_shot(hover);
                }
                let path = shoot(
                    &mut gpu,
                    4,
                    &format!("duel6-card-binder-{filter_name}-{name}"),
                );
                println!("{}", path.display());
                assert!(!overflowed(&gpu), "{filter_name} {name}");
                if let Some(console) = gpu.console.as_mut() {
                    console.collection_hover_for_shot(None);
                }
            }
            // Looking closely (20/10/2026): the Toxiee card, then a card with a short
            // lore and one with a long one, in the packs as they stand.
            for (id, label) in [
                ("sjk_alpha_010", "toxiee"),
                ("jka_characters_037", "short-lore"),
                ("sjk_blades_075", "long-lore"),
            ] {
                if id != "sjk_alpha_010" && crate::cards::faces::current().find(id).is_none() {
                    continue;
                }
                if let Some(console) = gpu.console.as_mut() {
                    console.collection_card_filter_for_shot(Filter::default());
                    console.collection_card_for_shot(binder_index(&list, id));
                    console.collection_inspect_for_shot(true);
                }
                let path = shoot(&mut gpu, 4, &format!("duel6-card-inspect-{label}-{name}"));
                println!("{}", path.display());
                assert!(!overflowed(&gpu), "inspect {label} {name}");
            }
            if let Some(console) = gpu.console.as_mut() {
                console.collection_inspect_for_shot(false);
            }
            if name == "4k" {
                continue;
            }
            // A missing card chosen with no filter: greyed up close, its hover card.
            if let Some(console) = gpu.console.as_mut() {
                console.collection_card_filter_for_shot(Filter::default());
                console.collection_card_for_shot(missing);
            }
            let _ = frame(&mut gpu, 4);
            if let Some(console) = gpu.console.as_mut() {
                console.collection_hover_for_shot(Some(missing));
            }
            let path = shoot(&mut gpu, 4, &format!("duel6-card-binder-missing-{name}"));
            println!("{}", path.display());
            assert!(!overflowed(&gpu), "missing {name}");
        }
    });
}

/// Mount the test packs (their pictures decoded as soon as a shot draws their face);
/// false (and a line) when the packs have no first card.
fn faces_ready() -> bool {
    super::saber_skins::mount_test_packs();
    if crate::cards::faces::current()
        .find("sjk_alpha_010")
        .is_some()
    {
        return true;
    }
    println!("no sjk_alpha_010 face in SJK_TEST_PACKS: skipped");
    false
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_card_faces() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let mut images = Vec::new();
        for finish in ["holo", "normal", "solar"] {
            let menu = menu::ClientMenu::new(true, String::new());
            let Some((mut gpu, _profile)) =
                open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &sjk_cvars())
            else {
                return;
            };
            let _ = frame(&mut gpu, 10);
            let list = with_face(finish);
            // The revealed card, wherever the binder puts it.
            let revealed = binder_index(&list, "sjk_alpha_010");
            if let Some(console) = gpu.console.as_mut() {
                console.preview_cards(list, 1_340, 85);
                console.preview_collection(Some(Vec::new()), 3.1);
            }
            gpu.open_profile_hub(Tab::Cards);
            if let Some(console) = gpu.console.as_mut() {
                console.collection_card_for_shot(revealed);
            }
            let _ = frame(&mut gpu, 120);
            let path = shoot(&mut gpu, 4, &format!("duel6-card-face-{finish}"));
            println!("{}", path.display());
            assert!(!overflowed(&gpu), "{finish}");
            images.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&images, 3, 640, "duel6-card-face-finishes").display()
        );
        // Tilted towards the pointer, up close: Holo, the pointer at the upper right.
        let menu = menu::ClientMenu::new(true, String::new());
        let Some((mut gpu, _profile)) =
            open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &sjk_cvars())
        else {
            return;
        };
        let _ = frame(&mut gpu, 10);
        let list = with_face("holo");
        let revealed = binder_index(&list, "sjk_alpha_010");
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(list, 1_340, 85);
            console.preview_collection(Some(Vec::new()), 3.1);
        }
        gpu.open_profile_hub(Tab::Cards);
        let mut tilts = Vec::new();
        for (name, at) in [("right", [0.7, -0.5]), ("left", [-0.8, 0.6])] {
            if let Some(console) = gpu.console.as_mut() {
                console.collection_card_for_shot(revealed);
                console.collection_tilt_for_shot(0, at);
            }
            let _ = frame(&mut gpu, 120);
            let path = shoot(&mut gpu, 4, &format!("duel6-card-face-tilt-{name}"));
            println!("{}", path.display());
            assert!(!overflowed(&gpu), "{name}");
            tilts.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&tilts, 2, 960, "duel6-card-face-tilts").display()
        );
        // Coming out of a Mythical Holocron.
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        let mut card = shown(3, 3, 1, 311);
        card.card = Some("sjk_alpha_010".to_owned());
        ceremony(&mut gpu, vec![card, shown(0, 0, 0, 14)], 3.0);
        let path = shoot(&mut gpu, 4, "duel6-card-face-opening");
        println!("{}", path.display());
        assert!(!overflowed(&gpu));
    });
}

/// The finishes in motion (19/10/2026, Sol's request): Solar's corona after the Sun
/// blade and Holo's smooth foil, up close on the Cards tab and on its binder tile at
/// three moments each, then Solar coming out of a holocron at three moments.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_card_finish_motion() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let moments = [1.0, 2.3, 3.7];
        for finish in ["solar", "holo"] {
            let menu = menu::ClientMenu::new(true, String::new());
            let Some((mut gpu, _profile)) =
                open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &sjk_cvars())
            else {
                return;
            };
            let _ = frame(&mut gpu, 10);
            let list = with_face(finish);
            let revealed = binder_index(&list, "sjk_alpha_010");
            if let Some(console) = gpu.console.as_mut() {
                console.preview_cards(list, 1_340, 85);
                console.preview_collection(Some(Vec::new()), moments[0]);
            }
            gpu.open_profile_hub(Tab::Cards);
            if let Some(console) = gpu.console.as_mut() {
                console.collection_card_for_shot(revealed);
            }
            let _ = frame(&mut gpu, 90);
            let mut images = Vec::new();
            for (index, seconds) in moments.iter().enumerate() {
                if let Some(console) = gpu.console.as_mut() {
                    console.preview_collection(Some(Vec::new()), *seconds);
                }
                let path = shoot(&mut gpu, 3, &format!("duel6-card-{finish}-{index}"));
                println!("{}", path.display());
                assert!(!overflowed(&gpu), "{finish} {index}");
                images.push(image::open(path).expect("the shot").to_rgba8());
            }
            println!(
                "{}",
                sheet(&images, 3, 640, &format!("duel6-card-{finish}-moments")).display()
            );
        }
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        let mut images = Vec::new();
        for (index, seconds) in moments.iter().enumerate() {
            let mut card = shown(3, 3, 2, 311);
            card.card = Some("sjk_alpha_010".to_owned());
            ceremony_at(&mut gpu, vec![card], 3.0, *seconds);
            let path = shoot(&mut gpu, 4, &format!("duel6-card-opening-solar-{index}"));
            println!("{}", path.display());
            assert!(!overflowed(&gpu));
            images.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&images, 3, 640, "duel6-card-opening-solar-moments").display()
        );
    });
}

/// Play `cards`' ceremony `elapsed` seconds into the first card's entrance, its finishes
/// drawn at `seconds`.
fn ceremony_at(gpu: &mut GpuState, cards: Vec<Shown>, elapsed: f32, seconds: f32) {
    if let Some(console) = gpu.console.as_mut() {
        let prizes = cards.into_iter().map(Prize::Card).collect();
        console.preview_opening(Some(prizes), elapsed, seconds);
    }
}

/// A made-up catalogue: the packs' own faces (`real`) first, then `sets` (id, name,
/// cards) of made-up ones.
fn made_up_faces(
    real: &[crate::cards::faces::Face],
    sets: &[(&str, &str, u16)],
) -> Vec<crate::cards::faces::Face> {
    let mut list = real.to_vec();
    let kinds = crate::cards::faces::Kind::ALL;
    for (set, set_name, count) in sets {
        for number in 1..=*count {
            let at = list.len();
            list.push(crate::cards::faces::Face {
                id: format!("{set}_{number:03}"),
                name: format!("{set_name} No. {number}"),
                set: (*set).to_owned(),
                set_name: (*set_name).to_owned(),
                number,
                kind: kinds[at % kinds.len()],
                rarity: &RARITIES[[0, 0, 1, 0, 2, 1, 0, 3, 1, 4][at % 10]],
                lore: "A made-up card for a world shot.".to_owned(),
                artist: "Sol".to_owned(),
                art: String::new(),
                grants: None,
                blade: None,
                scene: None,
                rank: None,
            });
        }
    }
    list
}

/// Copies of made-up card `id` of `rarity` in `finish`.
fn copy_of(id: &str, rarity: &str, finish: &str, count: u32) -> sjk_identity::CardStack {
    sjk_identity::CardStack {
        card: Some(id.to_owned()),
        rarity: rarity.to_owned(),
        finish: finish.to_owned(),
        edition: "alpha".to_owned(),
        count,
        first: 1_791_830_000,
        last: 1_791_830_000,
    }
}

/// Paging (19/10/2026): the binder with about sixty cards in three sets (the packs' own
/// and two made-up sets), scrolled partway with a tile in the middle chosen, in 1080p
/// and 4:3; then six sets, for the progress line.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_card_binder_many() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let packs = crate::cards::faces::current().list.clone();
        let real = packs.len().max(1);
        let three = [
            ("sjk_holonet", "Holonet Legends", 24_u16),
            ("sjk_duel", "Duel Masters", 22),
        ];
        let six = [
            ("sjk_holonet", "Holonet Legends", 24_u16),
            ("sjk_duel", "Duel Masters", 22),
            ("sjk_maps", "Map Makers", 9),
            ("sjk_force", "Force Echoes", 8),
            ("sjk_zz_tour", "Tournament", 6),
        ];
        let mut holdings = binder_holdings();
        for (index, number) in [1_u16, 2, 5, 8, 9, 13, 17, 21].into_iter().enumerate() {
            let finish = ["normal", "holo", "solar", "normal"][index % 4];
            holdings.push(copy_of(
                &format!("sjk_holonet_{number:03}"),
                "rare",
                finish,
                1,
            ));
        }
        for number in [3_u16, 4, 11, 20] {
            holdings.push(copy_of(
                &format!("sjk_duel_{number:03}"),
                "common",
                "normal",
                2,
            ));
        }
        holdings.push(copy_of("sjk_duel_012", "legendary", "solar", 1));
        for (sets, label) in [(&three[..], "3sets"), (&six[..], "6sets")] {
            for (size, name) in [([1920, 1080], "1080p"), ([1440, 1080], "4x3")] {
                if label == "6sets" && size[0] != 1920 {
                    continue;
                }
                let Some(mut gpu) = open_binder(size) else {
                    return;
                };
                // The packs' faces stay first, so their pictures keep their places.
                crate::cards::faces::install_for_shot(made_up_faces(&packs, sets), |index| {
                    index % real
                });
                if let Some(console) = gpu.console.as_mut() {
                    console.preview_cards(holdings.clone(), 1_340, 85);
                    console.preview_collection(Some(Vec::new()), 2.3);
                }
                let _ = frame(&mut gpu, 30);
                let chosen = binder_index(&holdings, "sjk_holonet_017");
                if let Some(console) = gpu.console.as_mut() {
                    console.collection_card_filter_for_shot(Filter::default());
                    console.collection_card_for_shot(chosen);
                }
                let path = shoot(
                    &mut gpu,
                    6,
                    &format!("duel6-card-binder-many-{label}-{name}"),
                );
                println!("{} (tile {chosen})", path.display());
                assert!(!overflowed(&gpu), "{label} {name}");
            }
        }
    });
}

/// A shader found by a holocron of tier `tier`: Tidal, held twice.
fn found_tidal(tier: usize) -> Prize {
    Prize::Shader(crate::console::holocrons_panel::opening::Found {
        holocron: 7,
        tier,
        skin: crate::unlockables::find("saber_tidal"),
        tag: None,
        id: "saber_tidal".to_owned(),
        name: "Tidal blade".to_owned(),
        count: 2,
    })
}

/// A shader newer than this client, from a Rare holocron.
fn found_new() -> Prize {
    Prize::Shader(crate::console::holocrons_panel::opening::Found {
        holocron: 9,
        tier: 1,
        skin: None,
        tag: None,
        id: "saber_comet".to_owned(),
        name: "Comet blade".to_owned(),
        count: 1,
    })
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_crate_spin() {
    on_big_stack(|| {
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        let mut moments = Vec::new();
        // The spin (negative: seconds before the burst), a card's and a shader's.
        for (elapsed, name, prize) in [
            (-4.2, "spin-start", found_tidal(1)),
            (-2.6, "spin-mid", found_tidal(1)),
            (-0.6, "spin-stop", found_tidal(1)),
            (3.0, "shader-words", found_tidal(1)),
            (-0.6, "card-stop", Prize::Card(shown(2, 3, 1, 96))),
            (-0.6, "new-stop", found_new()),
            (3.0, "new-words", found_new()),
        ] {
            if let Some(console) = gpu.console.as_mut() {
                console.preview_opening(Some(vec![prize]), elapsed, 3.1);
            }
            let path = shoot(&mut gpu, 2, &format!("duel6-crate-{name}"));
            assert!(!overflowed(&gpu), "{name}");
            println!("{}", path.display());
            moments.push(image::open(path).expect("the shot").to_rgba8());
        }
        println!(
            "{}",
            sheet(&moments, 2, 960, "duel6-crate-moments").display()
        );
    });
}

/// A shader `id` found by a holocron of tier `tier`, numbered `holocron`, held `count`.
fn found(tier: usize, holocron: u64, id: &str, count: u32) -> Prize {
    let skin = crate::unlockables::find(id);
    Prize::Shader(crate::console::holocrons_panel::opening::Found {
        holocron,
        tier,
        skin,
        tag: None,
        id: id.to_owned(),
        name: skin.map_or("A blade", |skin| skin.name).to_owned(),
        count,
    })
}

/// Twelve holocrons opened at once: cards of every finish and shaders, a few grand.
fn twelve() -> Vec<Prize> {
    (0..12_u64)
        .map(|at| {
            let tier = [0, 0, 1, 0, 2, 1, 0, 3, 1, 0, 2, 0][at as usize];
            match at {
                3 => found(tier, 40 + at, "saber_tidal", 2),
                5 => found(tier, 40 + at, "saber_nebula", 1),
                7 => found(tier, 40 + at, "saber_phoenix", 1),
                10 => found(tier, 40 + at, "saber_quicksilver", 1),
                _ => {
                    let rarity = [0, 1, 0, 0, 2, 0, 1, 3, 0, 0, 1, 0][at as usize];
                    let finish = [0, 0, 1, 0, 0, 0, 0, 2, 0, 1, 0, 0][at as usize];
                    Prize::Card(Shown {
                        holocron: 40 + at,
                        ..shown(tier, rarity, finish, 10 + at * 7)
                    })
                }
            }
        })
        .collect()
}

/// The opening as it plays since 17/10/2026: the 3D holocron gliding to the middle while
/// the hub answers, cracking open, the strip out of its flash with its bands and smear, the
/// prize lit at rest (a grand one with rays); then twelve opened at once, their strips
/// five at a time and everything found laid out.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_crate_open() {
    on_big_stack(|| {
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        let mut moments = Vec::new();
        let mut keep = |gpu: &mut GpuState, frames: u32, name: &str| {
            let path = shoot(gpu, frames, &format!("duel6-open-{name}"));
            assert!(!overflowed(gpu), "{name}");
            println!("{}", path.display());
            moments.push(image::open(path).expect("the shot").to_rgba8());
        };
        // The wait: the holocron glides to the middle and hums.
        if let Some(console) = gpu.console.as_mut() {
            console.preview_opening_state(None);
        }
        keep(&mut gpu, 50, "waiting");
        // One holocron, seconds before its entrance (the unseal is 1.7, the spin 6).
        let before = 7.7;
        for (elapsed, name, prize) in [
            (
                0.4 - before,
                "unseal-early",
                Prize::Card(shown(2, 2, 1, 96)),
            ),
            (
                1.1 - before,
                "unseal-charged",
                Prize::Card(shown(2, 2, 1, 96)),
            ),
            (
                1.55 - before,
                "unseal-flash",
                Prize::Card(shown(2, 2, 1, 96)),
            ),
            (-5.8, "strip-out-of-flash", Prize::Card(shown(2, 2, 1, 96))),
            (-4.6, "strip-fast", Prize::Card(shown(2, 2, 1, 96))),
            (-1.4, "strip-crawl", Prize::Card(shown(2, 2, 1, 96))),
            (-0.2, "strip-rest", Prize::Card(shown(2, 2, 1, 96))),
            (-0.2, "rest-solar", Prize::Card(shown(3, 3, 2, 311))),
            (-0.2, "rest-phoenix", found(3, 9, "saber_phoenix", 1)),
        ] {
            if let Some(console) = gpu.console.as_mut() {
                console.preview_opening(Some(vec![prize]), elapsed, 3.1);
            }
            // Long enough for the 3D holocron to grow back after the last flash.
            keep(&mut gpu, 40, name);
        }
        // Twelve at once (the batch's clock runs from its start).
        for (elapsed, name) in [
            (1.2, "batch-unseal"),
            (1.7 + 1.4, "batch-running"),
            (1.7 + 4.3, "batch-stopping"),
            (1.7 + 5.5, "batch-rest"),
            (1.7 + 5.9 * 2.0 + 5.5, "batch-last-round"),
            (1.7 + 5.9 * 3.0 + 0.4, "batch-found-coming"),
            (1.7 + 5.9 * 3.0 + 3.0, "batch-found"),
        ] {
            if let Some(console) = gpu.console.as_mut() {
                console.preview_opening(Some(twelve()), elapsed, 3.1);
            }
            keep(&mut gpu, 40, name);
        }
        println!(
            "{}",
            sheet(&moments, 3, 640, "duel6-open-moments").display()
        );
    });
}

/// The opening in other window shapes (4:3 and 21:9): the holocron cracking open in the
/// clear middle, five strips at rest and the finds laid out, all inside the window.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_crate_open_windows() {
    on_big_stack(|| {
        for (size, shape) in [([1440, 1080], "4x3"), ([2560, 1080], "21x9")] {
            let Some(mut gpu) = open_holocrons(size) else {
                return;
            };
            let mut moments = Vec::new();
            for (prizes, elapsed, name) in [
                (vec![Prize::Card(shown(2, 2, 1, 96))], 1.1 - 7.7, "unseal"),
                (twelve(), 1.7 + 5.5, "batch-rest"),
                (twelve(), 1.7 + 5.9 * 3.0 + 3.0, "batch-found"),
            ] {
                if let Some(console) = gpu.console.as_mut() {
                    console.preview_opening(Some(prizes), elapsed, 3.1);
                }
                let path = shoot(&mut gpu, 40, &format!("duel6-open-{shape}-{name}"));
                assert!(!overflowed(&gpu), "{shape} {name}");
                println!("{}", path.display());
                moments.push(image::open(path).expect("the shot").to_rgba8());
            }
            println!(
                "{}",
                sheet(&moments, 3, 640, &format!("duel6-open-{shape}")).display()
            );
        }
    });
}

/// The SJK Blades collection (20/10/2026) with a made-up blade card, "Sun blade" (the
/// packs have no blade card yet; its picture borrowed from SJK Alpha's): a Mythical
/// holocron's Solar copy of it opening, its blade's tile sliding out under the shards and
/// its words telling the blade it brought, in 1080p and 4:3; five opened at once with it
/// among the finds; and the card up close on the Cards tab with its hover card.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_blade_card() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let packs = crate::cards::faces::current().list.clone();
        let borrowed = packs
            .iter()
            .position(|face| face.id == "sjk_alpha_000")
            .unwrap_or(0);
        let mut list = packs.clone();
        list.push(crate::cards::faces::Face {
            id: "sjk_blades_007".to_owned(),
            name: "Sun blade".to_owned(),
            set: "sjk_blades".to_owned(),
            set_name: "SJK Blades".to_owned(),
            number: 7,
            kind: crate::cards::faces::Kind::Blade,
            rarity: &RARITIES[2],
            lore: "A made-up blade card for a world shot.".to_owned(),
            artist: "Sol".to_owned(),
            art: String::new(),
            grants: None,
            blade: Some("saber_sun".to_owned()),
            scene: None,
            rank: None,
        });
        let real = packs.len();
        crate::cards::faces::install_for_shot(
            list,
            |index| {
                if index < real { index } else { borrowed }
            },
        );
        let sun = crate::console::holocrons_panel::opening::Found {
            holocron: 61,
            tier: 3,
            skin: crate::unlockables::find("saber_sun"),
            tag: None,
            id: "saber_sun".to_owned(),
            name: "Sun blade".to_owned(),
            count: 1,
        };
        let blade_card = Shown {
            holocron: 61,
            shards: 24,
            card: Some("sjk_blades_007".to_owned()),
            bonus: Some(sun),
            ..shown(3, 2, 2, 312)
        };
        let mut moments = Vec::new();
        for (size, name) in [([1920, 1080], ""), ([1440, 1080], "-4x3")] {
            let Some(mut gpu) = open_holocrons(size) else {
                return;
            };
            let here: &[(f32, &str)] = if name.is_empty() {
                &[(2.3, "sliding"), (2.6, "counting"), (3.1, "words")]
            } else {
                &[(3.1, "words")]
            };
            for &(elapsed, moment) in here {
                ceremony(&mut gpu, vec![blade_card.clone()], elapsed);
                let path = shoot(&mut gpu, 40, &format!("duel6-blade-card-{moment}{name}"));
                println!("{}", path.display());
                assert!(!overflowed(&gpu), "{moment}{name}");
                moments.push(image::open(path).expect("the shot").to_rgba8());
            }
            if name.is_empty() {
                // Five at once, the blade card among them, laid out.
                let five = vec![
                    Prize::Card(blade_card.clone()),
                    Prize::Card(shown(1, 0, 0, 34)),
                    found_tidal(2),
                    Prize::Card(shown(0, 1, 1, 15)),
                    Prize::Card(shown(2, 0, 0, 90)),
                ];
                if let Some(console) = gpu.console.as_mut() {
                    console.preview_opening(Some(five), 1.7 + 5.9 + 3.0, 3.1);
                }
                let path = shoot(&mut gpu, 40, "duel6-blade-card-batch");
                println!("{}", path.display());
                assert!(!overflowed(&gpu), "batch");
                moments.push(image::open(path).expect("the shot").to_rgba8());
            }
        }
        // The Cards tab: the blade card held in Solar, up close with its hover card.
        let Some(mut gpu) = open_binder([1920, 1080]) else {
            return;
        };
        let mut holdings = binder_holdings();
        holdings.push(copy_of("sjk_blades_007", "rare", "solar", 1));
        let chosen = binder_index(&holdings, "sjk_blades_007");
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(holdings, 1_340, 85);
            console.preview_collection(Some(Vec::new()), 2.3);
        }
        let _ = frame(&mut gpu, 30);
        if let Some(console) = gpu.console.as_mut() {
            console.collection_card_filter_for_shot(Filter::default());
            console.collection_card_for_shot(chosen);
        }
        let _ = frame(&mut gpu, 4);
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(Some(chosen));
        }
        let path = shoot(&mut gpu, 4, "duel6-blade-card-binder");
        println!("{} (tile {chosen})", path.display());
        assert!(!overflowed(&gpu), "binder");
        moments.push(image::open(path).expect("the shot").to_rgba8());
        println!("{}", sheet(&moments, 3, 640, "duel6-blade-card").display());
    });
}

/// A ranked card (31/10/2026, the JoF collection): a made-up "Leader" card (Mythical
/// odds, cyan rank; its picture borrowed from SJK Alpha's) opening from a Mythical
/// holocron, its words naming the rank, then up close on the Cards tab with its hover
/// card. No rarity's name, mark or colour shows on it.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_sjk_ranked_card() {
    on_big_stack(|| {
        if !faces_ready() {
            return;
        }
        let packs = crate::cards::faces::current().list.clone();
        let borrowed = packs
            .iter()
            .position(|face| face.id == "sjk_alpha_000")
            .unwrap_or(0);
        let mut list = packs.clone();
        list.push(crate::cards::faces::Face {
            id: "ranked_001".to_owned(),
            name: "A Leader".to_owned(),
            set: "ranked".to_owned(),
            set_name: "Ranked".to_owned(),
            number: 1,
            kind: crate::cards::faces::Kind::Community,
            rarity: &RARITIES[4],
            lore: "A made-up ranked card for a world shot.".to_owned(),
            artist: "Sol".to_owned(),
            art: String::new(),
            grants: None,
            blade: None,
            scene: None,
            rank: Some(crate::cards::faces::Rank {
                name: "Leader".to_owned(),
                colour: sjk_ui::Color::new(
                    0x22 as f32 / 255.0,
                    0xD3 as f32 / 255.0,
                    0xEE as f32 / 255.0,
                    1.0,
                ),
            }),
        });
        let real = packs.len();
        crate::cards::faces::install_for_shot(
            list,
            |index| {
                if index < real { index } else { borrowed }
            },
        );
        let leader = Shown {
            holocron: 62,
            card: Some("ranked_001".to_owned()),
            ..shown(3, 4, 0, 312)
        };
        let mut moments = Vec::new();
        let Some(mut gpu) = open_holocrons([1920, 1080]) else {
            return;
        };
        ceremony(&mut gpu, vec![leader], 3.1);
        let path = shoot(&mut gpu, 40, "duel6-ranked-card-words");
        println!("{}", path.display());
        assert!(!overflowed(&gpu), "words");
        moments.push(image::open(path).expect("the shot").to_rgba8());
        drop(gpu);
        let Some(mut gpu) = open_binder([1920, 1080]) else {
            return;
        };
        let mut holdings = binder_holdings();
        holdings.push(copy_of("ranked_001", "mythical", "normal", 2));
        let chosen = binder_index(&holdings, "ranked_001");
        if let Some(console) = gpu.console.as_mut() {
            console.preview_cards(holdings, 1_340, 85);
            console.preview_collection(Some(Vec::new()), 2.3);
        }
        let _ = frame(&mut gpu, 30);
        if let Some(console) = gpu.console.as_mut() {
            console.collection_card_filter_for_shot(Filter::default());
            console.collection_card_for_shot(chosen);
        }
        let _ = frame(&mut gpu, 4);
        if let Some(console) = gpu.console.as_mut() {
            console.collection_hover_for_shot(Some(chosen));
        }
        let path = shoot(&mut gpu, 4, "duel6-ranked-card-binder");
        println!("{} (tile {chosen})", path.display());
        assert!(!overflowed(&gpu), "binder");
        moments.push(image::open(path).expect("the shot").to_rgba8());
        println!("{}", sheet(&moments, 2, 960, "duel6-ranked-card").display());
    });
}
