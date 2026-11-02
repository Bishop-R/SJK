//! Collection's static covers, ownership counts and authoritative equip action.
use super::*;
use crate::menu::sjk::{Frame, color, kit, text};
use crate::menu_widgets::TextFamily;
use crate::profile_card::banner::{self, ALL};
use sjk_ui::{DrawCommand, FontWeight, TextAlign};

impl Panel {
    pub(super) fn wear_banner(&self) -> PanelAction {
        if !self.banner_owned.get(self.banner).copied().unwrap_or(false) {
            return PanelAction::None;
        }
        let Some(chosen) = ALL.get(self.banner) else {
            return PanelAction::None;
        };
        if chosen.id.is_empty() && self.banner_current.is_empty() {
            return PanelAction::None;
        }
        PanelAction::Banner(if chosen.id == self.banner_current {
            ""
        } else {
            chosen.id
        })
    }

    pub(super) fn banners(&mut self, frame: &Frame, inputs: &Inputs<'_>) {
        let snapshot = inputs.snapshot.filter(|_| inputs.enabled);
        let profile = snapshot.and_then(|snapshot| snapshot.me.as_ref());
        let online =
            snapshot.is_some_and(|snapshot| snapshot.status == sjk_identity::Status::Online);
        self.banner = self.banner.min(ALL.len() - 1);
        self.banner_current = banner::equipped(profile);
        self.banner_owned = std::array::from_fn(|index| {
            online && profile.is_some() && banner::count(profile, ALL[index].id) > 0
        });
        self.lead(
            frame,
            "Profile banners",
            "Collectible profile covers. Preview any cover; equip one you own.",
        );
        for (index, cover) in ALL.iter().enumerate() {
            let x = 96.0 + (index % 2) as f32 * 440.0;
            let y = 304.0 + (index / 2) as f32 * 200.0;
            let rect = frame.rect(x, y, 412.0, 114.0);
            banner::draw(&mut self.ui, rect, cover.id);
            let count = banner::count(profile, cover.id);
            if count == 0 {
                let _ = self.ui.draw_list_mut().push(DrawCommand::SolidRect {
                    rect,
                    color: color::alpha(color::SPACE, 0.55),
                });
            }
            let _ = self.ui.draw_list_mut().push(DrawCommand::Border {
                rect,
                radius: 8.0 * frame.s,
                width: if index == self.banner { 2.0 } else { 1.0 } * frame.s,
                color: if index == self.banner {
                    color::GOLD_BRIGHT
                } else {
                    color::alpha(color::HOLO, 0.3)
                },
            });
            self.ui
                .hit_region(BANNER_BASE + index as u16, frame.rect(x, y, 412.0, 170.0));
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!("{}", cover.name),
                frame.rect(x, y + 124.0, 290.0, 26.0),
                22.0 * frame.s,
                color::TEXT,
                FontWeight::Semibold,
                TextAlign::Start,
            );
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!(
                    "{}  x{}{}",
                    cover.tier.label(),
                    count,
                    if cover.id == self.banner_current {
                        "  Equipped"
                    } else {
                        ""
                    }
                ),
                frame.rect(x, y + 152.0, 412.0, 24.0),
                16.0 * frame.s,
                cover.tier.colour(),
                FontWeight::Regular,
                TextAlign::Start,
            );
        }
        let chosen = &ALL[self.banner];
        banner::draw(
            &mut self.ui,
            frame.rect(1050.0, 304.0, 740.0, 206.0),
            chosen.id,
        );
        let name = profile.map_or("Your profile", |profile| profile.name.as_str());
        crate::profile_card::avatar(
            &mut self.ui,
            frame.point(1120.0, 506.0),
            52.0 * frame.s,
            &crate::profile_card::Avatar {
                key_id: profile.map_or("", |profile| profile.key_id.as_str()),
                version: profile.map_or("", |profile| profile.avatar.as_str()),
                name,
                verified: profile.is_some_and(|profile| profile.verified),
                tick: profile.map_or(sjk_identity::Tick::None, |profile| profile.tick),
                trust: profile.and_then(|profile| profile.trust),
                preview: false,
                lit: false,
            },
        );
        text(
            &mut self.ui,
            TextFamily::Display,
            format_args!("{name}"),
            frame.rect(1200.0, 476.0, 570.0, 50.0),
            30.0 * frame.s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        text(
            &mut self.ui,
            TextFamily::Display,
            format_args!("{}", chosen.name),
            frame.rect(1050.0, 600.0, 740.0, 48.0),
            38.0 * frame.s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        text(
            &mut self.ui,
            TextFamily::Body,
            format_args!(
                "{} cover  |  {} copies",
                chosen.tier.label(),
                banner::count(profile, chosen.id)
            ),
            frame.rect(1050.0, 660.0, 740.0, 28.0),
            20.0 * frame.s,
            chosen.tier.colour(),
            FontWeight::Regular,
            TextAlign::Start,
        );
        let usable = self.banner_owned[self.banner]
            && (!chosen.id.is_empty() || !self.banner_current.is_empty());
        let label = if !online {
            "Connect to equip"
        } else if banner::count(profile, chosen.id) == 0 {
            "Locked"
        } else if chosen.id == self.banner_current && !chosen.id.is_empty() {
            "Unequip"
        } else if chosen.id.is_empty() && self.banner_current.is_empty() {
            "Equipped"
        } else {
            "Equip"
        };
        kit::button(
            &mut self.ui,
            frame,
            [1050.0, 740.0, 260.0, 54.0],
            label,
            true,
            usable,
            false,
            BANNER_WEAR,
        );
        let notice = snapshot
            .and_then(|snapshot| snapshot.notice.as_deref())
            .unwrap_or("Banners are cosmetic. The neutral SJK cover is always available.");
        for (row, words) in crate::menu::sjk::wrap(notice, 68).enumerate() {
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!("{words}"),
                frame.rect(1050.0, 830.0 + row as f32 * 26.0, 740.0, 26.0),
                18.0 * frame.s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::Start,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locked_selection_never_equips_and_owned_selection_toggles() {
        let mut panel = Panel::new();
        panel.banner = 1;
        assert!(matches!(panel.wear_banner(), PanelAction::None));
        panel.banner_owned[1] = true;
        assert!(matches!(
            panel.wear_banner(),
            PanelAction::Banner("banner_jedi_archives")
        ));
        panel.banner_current = ALL[1].id;
        assert!(matches!(panel.wear_banner(), PanelAction::Banner("")));
        panel.banner = 2;
        assert!(matches!(panel.wear_banner(), PanelAction::None));
        panel.banner = 0;
        panel.banner_owned[0] = true;
        assert!(matches!(panel.wear_banner(), PanelAction::Banner("")));
        panel.banner_current = "";
        assert!(matches!(panel.wear_banner(), PanelAction::None));
    }
    #[test]
    fn the_offline_picker_previews_every_cover_but_cannot_equip() {
        let inputs = super::super::tests::inputs(Holdings::Known(&[]), "");
        let panel = super::super::tests::drawn(Tab::Banners, &inputs);
        assert!(!panel.overflowed());
        for index in 0..ALL.len() {
            assert!(panel.ui.rect_for(BANNER_BASE + index as u16).is_some());
        }
        assert!(matches!(panel.wear_banner(), PanelAction::None));
    }
}
