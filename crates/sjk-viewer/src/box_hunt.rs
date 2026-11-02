//! Box hunt, SJK's mystery box game (`toy_boxhunt`; Sol's design, 27/10/2026): once
//! every 10 minutes at most the SJK hub picks one player who has the toy on as the mark;
//! another player with it on who kills the mark wins a free mystery box roll.
//!
//! - **Opt-in:** the switch (`toy_boxhunt`, the Collection's Toys tab, the key bindings)
//!   is the consent to take part: only players with it on see the mark or can be it. It
//!   has no key of its own beyond the switch, and stays as last switched
//!   ([`crate::toy_saved::BOX_HUNT_CVAR`]).
//! - **Told:** the switch travels in the player's look ([`TOY`] among its toys), which is
//!   what the hub reads when it picks the mark.

use crate::GpuState;
use std::time::Instant;

/// The toy's console command: the switch (take part, or not).
pub(crate) const COMMAND: &str = "toy_boxhunt";
/// The toy's id in looks: the hub picks the mark among the players whose look has it.
pub(crate) const TOY: &str = "box_hunt";
/// The toy's icon (quick wheel, Collection, key bindings).
pub(crate) const ICON: &str = "gfx/sjk/toy_boxhunt.png";

/// The bundled files at their game paths.
const FILES: [(&str, &[u8]); 1] = [(
    ICON,
    include_bytes!("../assets/mystery_box/toy_boxhunt.png"),
)];

/// Mount the bundled files.
pub(crate) fn mount(vfs: &mut sjk_vfs::VirtualFileSystem) -> Result<(), sjk_vfs::VfsError> {
    vfs.mount_memory("SJK Box hunt", FILES.iter().copied())?;
    Ok(())
}

/// The toy's state.
#[derive(Default)]
pub(crate) struct BoxHunt {
    on: bool,
}

impl BoxHunt {
    /// `toy_boxhunt`: on or off.
    pub(crate) fn toggle(&mut self) {
        self.on = !self.on;
    }

    /// On (`true`) or off.
    pub(crate) fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Whether the toy is on.
    pub(crate) fn on(&self) -> bool {
        self.on
    }
}

impl GpuState {
    /// `toy_boxhunt`: on or off, said on the centre of the screen.
    pub(crate) fn toggle_box_hunt(&mut self) {
        self.box_hunt.toggle();
        self.chat.receive(
            sjk_client::ServerEventKind::CenterPrint,
            crate::toy_switch_words::switched("Box hunt", self.box_hunt.on()),
            None,
            Instant::now(),
        );
    }
}
