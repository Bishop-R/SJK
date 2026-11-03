//! Moving with the console open (`cl_consoleMove`, AngeloPapaya's idea, 03/11/2026;
//! Settings > Controls "Move with console open", off by default as stock).
//!
//! Stock lets go of every key when the console opens (`Key_ClearStates`) and sends
//! `BUTTON_TALK` while it is open (`CL_CmdButtons`), which the server's movement reads
//! as "stand still" (`PmoveSingle` zeroes the command's moves). With the option on:
//!
//! - the movement keys held as the console opens (forward, back, strafing, jump,
//!   crouch, walk) keep their hold ([`crate::input::GameplayInput::release_keys_but_movement`]),
//!   and letting one go stops it, as the releases reach the bindings wherever the
//!   keyboard is; attack, use and the other buttons are let go as before;
//! - their key repeats are not typed into the input line;
//! - commands made while the console is the only thing open leave `BUTTON_TALK` out,
//!   so the player moves and no talk balloon shows over them.
//!
//! A key pressed while the console is open types, as before: the console keeps the
//! keyboard, so a command starting with w, a, s or d types as it should. To change
//! direction, close the console. Nothing here sends what a player could not: the same
//! commands come from the same keys with the console closed.

use crate::console::ViewerConsole;

/// The setting.
pub(crate) const CVAR: &str = "cl_consoleMove";

/// Whether the option is on.
pub(crate) fn enabled(console: Option<&ViewerConsole>) -> bool {
    console.and_then(|console| console.bool_cvar(CVAR)) == Some(true)
}

impl crate::GpuState {
    /// The console is open over a match and the movement keys keep working.
    pub(crate) fn console_keeps_moving(&self) -> bool {
        self.live_session.is_some()
            && self
                .console
                .as_ref()
                .is_some_and(|console| console.is_open() && enabled(Some(console)))
    }
}
