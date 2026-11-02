//! Offline review of the shared game-chat logging setting in SJK UI.
use super::*;

#[test]
#[ignore = "renders with the GPU and installed data named by JKA_GAME_DATA"]
fn duel6_settings_chat_logs() {
    on_big_stack(|| {
        let cvars = [
            ("ui_menuStyle", "sjk"),
            (crate::settings::quick::HIDE_CVAR, "1"),
            ("cl_identity", "0"),
            ("cl_discordPresence", "0"),
        ];
        let Some((mut gpu, _profile)) = open(
            "maps/mp/duel6.bsp",
            [1920, 1080],
            Some(menu::ClientMenu::new(true, String::new())),
            &cvars,
        ) else {
            return;
        };
        let _ = frame(&mut gpu, 4);
        if let (Some(menu), Some(console)) = (gpu.client_menu.as_mut(), gpu.console.as_ref()) {
            menu.open_sjk_settings(console, 9, crate::player_menu::ReturnTarget::MainMenu);
            menu.select_setting_for_shot(crate::console::chat_log::CVAR);
        }
        println!("{}", shoot(&mut gpu, 6, "chat-logs-settings").display());
    });
}
