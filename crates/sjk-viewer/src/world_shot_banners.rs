//! Offline screenshots of the approved static banner collection and profile header.
use super::*;
use crate::player_menu::ReturnTarget;
use crate::profile_hub::Tab;

#[test]
#[ignore = "needs a GPU and installed game data named by JKA_GAME_DATA"]
fn duel6_profile_banners() {
    on_big_stack(|| {
        let cvars = [
            ("ui_menuStyle", "sjk"),
            ("cl_discordPresence", "0"),
            ("cl_identity", "0"),
        ];
        let Some((mut gpu, _profile)) = open(
            "maps/mp/duel6.bsp",
            [1920, 1080],
            Some(menu::ClientMenu::new(true, String::new())),
            &cvars,
        ) else {
            return;
        };
        let _ = frame(&mut gpu, 2);
        let mut preview = super::tests::profile_preview();
        let banners = &crate::profile_card::banner::ALL;
        let owned: Vec<sjk_identity::Unlock> = banners
            .iter()
            .skip(1)
            .filter(|banner| banner.id != "banner_living_nebula")
            .map(|banner| sjk_identity::Unlock {
                id: banner.id.to_owned(),
                granted: 1_791_336_225,
                note: String::new(),
                count: if banner.id == "banner_hyperspace" {
                    2
                } else {
                    1
                },
                medal: None,
                card: None,
                holo: 0,
                solar: 0,
            })
            .collect();
        let me = preview.snapshot.me.as_mut().unwrap();
        me.name = "^3Banner Explorer".to_owned();
        me.banner = "banner_hyperspace".into();
        me.unlocks = owned.clone();
        let console = gpu.console.as_mut().unwrap();
        console.preview_profile(preview);
        console.preview_collection(Some(owned), 0.8);
        console.open_profile_hub_page(Tab::Banners, ReturnTarget::MainMenu);
        console.collection_banner_for_shot(4);
        println!(
            "{}",
            shoot(&mut gpu, 6, "profile-banners-collection").display()
        );
        assert!(!gpu.console.as_ref().unwrap().profile_pages_overflowed());
        gpu.console
            .as_mut()
            .unwrap()
            .open_profile_hub_page(Tab::Profile, ReturnTarget::MainMenu);
        assert_eq!(gpu.profile_hub_tab(), Some(Tab::Profile));
        println!(
            "{}",
            shoot(&mut gpu, 6, "profile-banners-profile").display()
        );
        assert!(!gpu.console.as_ref().unwrap().profile_pages_overflowed());
        gpu.console
            .as_mut()
            .unwrap()
            .open_profile_hub_page(Tab::Holocrons, ReturnTarget::MainMenu);
        assert_eq!(gpu.profile_hub_tab(), Some(Tab::Holocrons));
        let console = gpu.console.as_mut().unwrap();
        console.preview_holocrons(super::holocrons_tab::holdings(), 2);
        console.preview_opening(
            Some(vec![
                crate::console::holocrons_panel::opening::Prize::Shader(
                    crate::console::holocrons_panel::opening::Found {
                        holocron: 1,
                        tier: 2,
                        skin: None,
                        tag: None,
                        id: "banner_hyperspace".to_owned(),
                        name: "Hyperspace".to_owned(),
                        count: 2,
                        unknown: "profile banner",
                    },
                ),
            ]),
            4.0,
            4.0,
        );
        println!(
            "{}",
            shoot(&mut gpu, 6, "profile-banners-opening").display()
        );
        assert!(!gpu.console.as_ref().unwrap().profile_pages_overflowed());
    });
}
