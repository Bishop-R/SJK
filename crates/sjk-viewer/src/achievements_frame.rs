//! The per-frame side of achievements (`achievements.rs`): each live snapshot goes to
//! the tracker with the obituaries it brought, and twice a second (with the identity's
//! turn, `identity_frame.rs`) the counts are loaded, synced with the hub, saved, and
//! new unlocks announced with a console line and the pop-up (`unlock_toast.rs`): with
//! the hub on, one the client counts only once the hub unlocked it.

use super::*;

/// Count what the live `snapshot` of `session` shows for the achievements;
/// `obituaries_before` is the obituary tracker's count before the snapshot was observed;
/// `boxed` whether the player hides in the cardboard box, `toys` what the toys tell of
/// the moment ([`GpuState::toy_moments`]).
pub(crate) fn observe(
    tracker: &mut achievements::tracker::Tracker,
    obituaries: &sjk_client::ObituaryTracker,
    session: &ClientSession,
    snapshot: &Snapshot,
    obituaries_before: u64,
    boxed: bool,
    toys: achievements::tracker::Toys,
    now: Instant,
) {
    // The feed keeps the newest eight; more in one snapshot than that is unheard of.
    let new = (obituaries.decoded() - obituaries_before).min(8) as usize;
    let mut events = [None; 8];
    for (slot, offset) in events.iter_mut().zip((0..new).rev()) {
        *slot = obituaries.feed().newest(offset);
    }
    let events: Vec<sjk_client::ObituaryEvent> = events.into_iter().flatten().collect();
    let seen = achievements::tracker::Seen {
        snapshot,
        game_state: session.game_state(),
        server: session.server(),
        local: session.is_local(),
        obituaries: &events,
        boxed,
        toys,
        now,
    };
    achievements::update(|record| tracker.observe(&seen, record));
}

impl GpuState {
    /// What the toys tell the tracker of this moment, for their achievements.
    pub(crate) fn toy_moments(&self) -> achievements::tracker::Toys {
        achievements::tracker::Toys {
            lifted_by: self.cardboard_box.lifted_by(),
            steering: self.hax.steering().map(u16::from),
            landed: self.drone.landed(),
            listeners: self.boombox.listeners(),
        }
    }

    /// Whether what a toy sees counts towards achievements: in a live match on a server
    /// of another machine, as the tracker's (`achievements/tracker.rs`).
    pub(crate) fn toys_count(&self) -> bool {
        self.live_session
            .as_ref()
            .is_some_and(|session| !session.is_local() && !session.server().ip().is_loopback())
    }

    /// Twice a second: load the counts once, take in what the hub holds, hand the
    /// service the counts, save, and announce what was unlocked.
    pub(crate) fn update_achievements(&mut self) {
        let Some(console) = self.console.as_mut() else {
            return;
        };
        achievements::load(console.config_directory());
        let held = player_identity::own_achievements();
        achievements::sync(held.as_deref(), player_identity::hub_on(), false);
        for achievements::Announcement { kind, crystal } in achievements::take_announcements() {
            console.push_log(format!(
                "^3Achievement unlocked: ^7{} ^5({})",
                kind.name, kind.description
            ));
            self.unlock_toast
                .push(crate::unlock_toast::Unlock::Achievement { kind, crystal });
        }
    }
}
