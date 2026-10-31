//! The SJK UI Saber page's Sounds row (`docs/unlockables.md`, "Saber sounds"): the
//! saber sounds the player is heard with, apart from the shader they wear. Follow the
//! shader (the default), the stock sounds, then the sound set of every saber shader the
//! player's hub profile lists, the rarest tier first as the Blade row lists them (the
//! same holdings, [`super::blade_skins::BladeChoice::owned`]). Picking one writes
//! `cg_saberSounds`; Preview (or Enter on the row) plays the choice's ignition and hum.

use super::blade_skins::BladeChoice;
use crate::console::ViewerConsole;
use crate::saber_sound_choice::{CVAR, SoundChoice};

/// What the Sounds row offers and which it shows chosen.
pub(super) struct SoundsChoice {
    /// Follow, Stock, then the owned shaders' sound sets.
    choices: Vec<SoundChoice>,
    /// The choice `cg_saberSounds` names, while it is offered; else Follow.
    chosen: usize,
}

impl SoundsChoice {
    pub(super) fn new() -> Self {
        Self {
            choices: vec![SoundChoice::Follow, SoundChoice::Stock],
            chosen: 0,
        }
    }

    /// Take the shaders `blade` offers (the owned ones) and the setting.
    pub(super) fn read(&mut self, console: &ViewerConsole, blade: &BladeChoice) {
        self.choices.truncate(2);
        self.choices.extend(
            blade
                .owned()
                .iter()
                .map(|skin| SoundChoice::Shader(skin.id)),
        );
        let setting = SoundChoice::parse(console.text_value(CVAR).unwrap_or_default());
        self.chosen = self
            .choices
            .iter()
            .position(|choice| *choice == setting)
            .unwrap_or(0);
    }

    /// How many choices there are.
    pub(super) fn count(&self) -> usize {
        self.choices.len()
    }

    /// The choice shown chosen.
    pub(super) fn chosen(&self) -> SoundChoice {
        self.choices[self.chosen]
    }

    /// Make choice `index` the sounds and write it, unless it is the one shown chosen
    /// (so a shader's sounds chosen before the hub answered are kept).
    pub(super) fn pick(&mut self, console: &mut ViewerConsole, index: usize) {
        let Some(choice) = self.choices.get(index).copied() else {
            return;
        };
        if index == self.chosen {
            return;
        }
        console.set_cvar(CVAR, choice.wire());
        self.chosen = index;
    }

    /// Move the choice `direction` along, wrapping, and write it.
    pub(super) fn step(&mut self, console: &mut ViewerConsole, direction: isize) {
        let next = super::controller::wrap(self.chosen, direction, self.count());
        self.pick(console, next);
    }

    /// The blade skin whose sounds the chosen one plays for a player wearing `blade`
    /// (`None` the stock sounds): what Preview plays.
    pub(super) fn heard(&self, blade: Option<&'static str>) -> Option<&'static str> {
        self.chosen().sound_skin(blade)
    }

    /// The chosen one's name; following the shader names the one heard.
    pub(super) fn label(&self, blade: Option<&'static crate::unlockables::Unlockable>) -> String {
        match (self.chosen(), blade) {
            (SoundChoice::Follow, Some(blade)) => format!("Follow shader ({})", blade.name),
            (SoundChoice::Follow, None) => "Follow shader (stock)".to_owned(),
            (choice, _) => choice.label().to_owned(),
        }
    }

    /// Play the chosen sounds' preview for a player wearing `blade`.
    pub(super) fn preview(&self, blade: Option<&'static str>) {
        crate::saber_sound_choice::post_preview(self.heard(blade));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unlock(id: &str) -> sjk_identity::Unlock {
        sjk_identity::Unlock {
            holo: 0,
            solar: 0,
            id: id.to_owned(),
            granted: 1_791_336_225,
            note: String::new(),
            count: 1,
            medal: None,
            card: None,
        }
    }

    /// Follow and Stock, then the owned shaders' sounds as the Blade row lists them;
    /// picking and stepping write `cg_saberSounds`, a shader not owned (any more) shows
    /// Follow, and what Preview plays follows the blade only while following.
    #[test]
    fn follow_stock_and_the_owned_shaders_are_offered_and_picking_writes_the_setting() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        let mut blade = BladeChoice::new();
        blade.preview = Some(vec![unlock("saber_void"), unlock("saber_sun")]);
        blade.read(&console);
        let mut sounds = SoundsChoice::new();
        sounds.read(&console, &blade);
        assert_eq!(sounds.count(), 4);
        assert_eq!(sounds.chosen(), SoundChoice::Follow);
        assert_eq!(sounds.heard(Some("saber_sun")), Some("saber_sun"));
        sounds.pick(&mut console, 1);
        assert_eq!(console.text_value(CVAR), Some("stock"));
        assert_eq!(sounds.heard(Some("saber_sun")), None);
        sounds.step(&mut console, 1);
        let shader = sounds.chosen();
        assert!(matches!(shader, SoundChoice::Shader(_)));
        assert_eq!(console.text_value(CVAR), Some(shader.wire()));
        assert_eq!(sounds.heard(None), shader.sound_skin(None));
        sounds.read(&console, &blade);
        assert_eq!(sounds.chosen(), shader, "read back from the setting");
        sounds.step(&mut console, -2);
        assert_eq!(console.text_value(CVAR), Some(""), "back to follow");
        // A shader's sounds no longer owned: Follow shows, the setting kept.
        console.set_cvar(CVAR, "saber_storm");
        sounds.read(&console, &blade);
        assert_eq!(sounds.chosen(), SoundChoice::Follow);
        sounds.pick(&mut console, 0);
        assert_eq!(console.text_value(CVAR), Some("saber_storm"));
    }
}
