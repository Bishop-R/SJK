//! Off-screen review of reaction controls using synthetic public chat data only.
use super::*;

#[test]
#[ignore = "renders with the GPU and installed data named by JKA_GAME_DATA"]
fn duel6_sjk_chat_reactions() {
    on_big_stack(|| {
        let cvars = [
            ("ui_menuStyle", "sjk"),
            ("cl_identity", "0"),
            ("cl_discordPresence", "0"),
            ("cl_sjkChat", "1"),
        ];
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", [1920, 1080], None, &cvars)
        else {
            return;
        };
        let mut state = sjk_identity::ChatState {
            live: true,
            online: 12,
            loaded: Some(1),
            own_key: Some(*b"0123456789abcdef"),
            ..Default::default()
        };
        for (id, name, text) in [
            (1, "^3Padawan", "Anyone up for a duel?"),
            (2, "^5Blue Fox", "That was a great round!"),
            (3, "^2Jedi", "See you in the courtyard."),
        ] {
            let mut message: sjk_identity::ChatMessage =
                serde_json::from_value(serde_json::json!({
                    "id":id,"at":1793367000,"key_id":format!("{id:016x}"),"name":name,"text":text,
                    "client":"sjk"
                }))
                .unwrap();
            if id == 2 {
                message.reactions = vec![
                    sjk_identity::reactions::Reaction {
                        key_id: "0123456789abcdef".into(),
                        emoji: "heart".into(),
                    },
                    sjk_identity::reactions::Reaction {
                        key_id: "aaaaaaaaaaaaaaaa".into(),
                        emoji: "heart".into(),
                    },
                    sjk_identity::reactions::Reaction {
                        key_id: "bbbbbbbbbbbbbbbb".into(),
                        emoji: "fire".into(),
                    },
                ]
                .into();
            }
            state.messages.push_back(message);
        }
        state.revision = 1;
        gpu.console
            .as_mut()
            .unwrap()
            .preview_sjk_chat(state, Some(2));
        println!(
            "{}",
            shoot(&mut gpu, 12, "duel6-sjk-chat-reactions").display()
        );
    });
}
