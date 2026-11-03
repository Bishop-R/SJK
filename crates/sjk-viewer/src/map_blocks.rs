//! Blocking for this map (`docs/hub-chat.md`, "Blocking for this map"): players blocked
//! until the server's map changes, as the PC-wide block does (`muted_players.rs`: Kyle,
//! silent, named BLOCKED), by this player alone or, by SJK staff, for every SJK player
//! there who follows staff blocks (`cl_staffBlocks`, `PROTOCOL.md` "Map blocks").
//!
//! A block names a slot and the name the game showed there. It follows that player
//! while they keep the slot, through renames, and ends when the slot empties (they
//! left) or the map load (`sv_serverid`) changes. A staff block reaches this client
//! with the hub's presence; it applies while the name in its slot matches the one the
//! staff saw, or the player this client saw it match renamed since. Lifting a staff
//! block here (`unblock`) lifts it for this player only, for the rest of the map.

use crate::chat_mutes::SLOTS;
use sjk_identity::{MapBlock, names_match};

/// `cl_staffBlocks`: whether staff blocks apply here (on by default).
pub(crate) const CVAR: &str = "cl_staffBlocks";

/// The map blocks of the server being played.
#[derive(Debug, Default)]
pub(crate) struct MapBlocks {
    /// The map load (`sv_serverid`) the blocks are for; `None` off a server.
    serverid: Option<i32>,
    /// Whether staff blocks apply here (`cl_staffBlocks`).
    follow: bool,
    /// The players this player blocked for the map, by slot: the name they wear now.
    own: [Option<String>; SLOTS],
    /// The players staff blocked that this client matched, by slot: the name they
    /// wear now, so a rename keeps the block.
    shared: [Option<String>; SLOTS],
    /// Slots whose staff block this player lifted for themselves.
    excepted: u32,
    /// The blocked slots, as last worked out: this player's and staff's.
    own_slots: u32,
    shared_slots: u32,
    /// Counts changes to what the slots are worked out from, but the roster.
    revision: u64,
}

impl MapBlocks {
    /// The map load being played (`None` off a server) and whether staff blocks
    /// apply; a new map load forgets every block.
    pub(crate) fn enter(&mut self, serverid: Option<i32>, follow: bool) {
        if self.serverid != serverid {
            self.serverid = serverid;
            self.own = Default::default();
            self.shared = Default::default();
            self.excepted = 0;
            self.revision += 1;
        }
        if self.follow != follow {
            self.follow = follow;
            self.revision += 1;
        }
    }

    /// Counts changes, for [`crate::muted_players::MutedPlayers::refresh`].
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// Block the player in `slot`, shown as `name`, for this player until the map
    /// changes; false off a server or for a slot out of range.
    pub(crate) fn block(&mut self, slot: usize, name: &str) -> bool {
        if self.serverid.is_none() || slot >= SLOTS {
            return false;
        }
        self.own[slot] = Some(name.to_owned());
        self.excepted &= !(1 << slot);
        self.revision += 1;
        true
    }

    /// Lift the blocks on `slot` here: this player's, and a staff block for this
    /// player alone. Returns whether one was lifted.
    pub(crate) fn unblock(&mut self, slot: usize) -> bool {
        if slot >= SLOTS {
            return false;
        }
        let own = self.own[slot].take().is_some();
        let shared = self.shared_slots & (1 << slot) != 0;
        if shared {
            self.excepted |= 1 << slot;
        }
        if own || shared {
            self.revision += 1;
        }
        own || shared
    }

    /// Lift every block here; returns how many slots were blocked.
    pub(crate) fn clear(&mut self) -> usize {
        let own = (0..SLOTS)
            .filter(|slot| self.own[*slot].is_some())
            .fold(0u32, |slots, slot| slots | 1 << slot);
        let count = (own | self.shared_slots).count_ones() as usize;
        self.own = Default::default();
        self.excepted |= self.shared_slots;
        self.revision += 1;
        count
    }

    /// Whether this player blocked anyone for the map (the slots may need working out).
    pub(crate) fn pending(&self) -> bool {
        self.own.iter().any(Option::is_some) || self.own_slots != 0
    }

    /// The slots this player blocked for the map, as last worked out.
    pub(crate) fn own_slots(&self) -> u32 {
        self.own_slots
    }

    /// The slots blocked by staff for everyone, as last worked out.
    pub(crate) fn shared_slots(&self) -> u32 {
        self.shared_slots
    }

    /// Work the blocked slots out from `roster`, what the game shows in each slot
    /// (empty for none), the player's own slot and `staff`, the staff blocks the hub
    /// gives for this server. Returns the blocked slots.
    pub(crate) fn resolve(
        &mut self,
        roster: &[&str; SLOTS],
        own_slot: Option<usize>,
        staff: &[MapBlock],
    ) -> u32 {
        let mut own_slots = 0;
        for (slot, entry) in self.own.iter_mut().enumerate() {
            let shown = roster[slot];
            match entry {
                // They left: the slot is someone else's next.
                Some(_) if shown.is_empty() => *entry = None,
                Some(name) => {
                    if name != shown {
                        shown.clone_into(name);
                    }
                    own_slots |= 1 << slot;
                }
                None => {}
            }
        }
        let mut listed = 0u32;
        let mut shared_slots = 0;
        if self.follow {
            for block in staff {
                let slot = usize::from(block.slot);
                if slot >= SLOTS || Some(block.serverid) != self.serverid.map(i64::from) {
                    continue;
                }
                listed |= 1 << slot;
                let shown = roster[slot];
                if shown.is_empty() {
                    self.shared[slot] = None;
                    continue;
                }
                let followed = match &self.shared[slot] {
                    // The player matched before, renamed or not.
                    Some(_) => true,
                    None => names_match(&block.name, shown),
                };
                if followed {
                    self.shared[slot] = Some(shown.to_owned());
                    if self.excepted & (1 << slot) == 0 {
                        shared_slots |= 1 << slot;
                    }
                }
            }
        }
        for (slot, followed) in self.shared.iter_mut().enumerate() {
            if listed & (1 << slot) == 0 {
                *followed = None;
            }
        }
        // An exception lasts while the slot's player stays.
        for (slot, shown) in roster.iter().enumerate() {
            if shown.is_empty() {
                self.excepted &= !(1 << slot);
            }
        }
        if let Some(own) = own_slot.filter(|slot| *slot < SLOTS) {
            own_slots &= !(1 << own);
            shared_slots &= !(1 << own);
        }
        self.own_slots = own_slots;
        self.shared_slots = shared_slots;
        own_slots | shared_slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster(names: &[(usize, &'static str)]) -> [&'static str; SLOTS] {
        let mut roster = [""; SLOTS];
        for (slot, name) in names {
            roster[*slot] = name;
        }
        roster
    }

    fn staff(slot: u8, name: &str, serverid: i64) -> MapBlock {
        MapBlock {
            map: "mp/ffa3".to_owned(),
            serverid,
            slot,
            name: name.to_owned(),
            at: 0,
        }
    }

    #[test]
    fn an_own_block_follows_the_player_until_they_leave_or_the_map_changes() {
        let mut blocks = MapBlocks::default();
        assert!(!blocks.block(4, "Troll"), "off a server");
        blocks.enter(Some(7), true);
        assert!(blocks.block(4, "^1Troll"));
        let here = roster(&[(1, "Me"), (4, "^1Troll")]);
        assert_eq!(blocks.resolve(&here, Some(1), &[]), 1 << 4);
        // A rename keeps it.
        let renamed = roster(&[(1, "Me"), (4, "NotTroll")]);
        assert_eq!(blocks.resolve(&renamed, Some(1), &[]), 1 << 4);
        // They left: someone new in the slot is not blocked.
        assert_eq!(blocks.resolve(&roster(&[(1, "Me")]), Some(1), &[]), 0);
        assert_eq!(blocks.resolve(&here, Some(1), &[]), 0);
        // A new map load forgets the blocks.
        blocks.block(4, "^1Troll");
        assert_eq!(blocks.resolve(&here, Some(1), &[]), 1 << 4);
        let before = blocks.revision();
        blocks.enter(Some(8), true);
        assert!(blocks.revision() > before);
        assert_eq!(blocks.resolve(&here, Some(1), &[]), 0);
        // Never the player's own slot.
        blocks.block(1, "Me");
        assert_eq!(blocks.resolve(&here, Some(1), &[]), 0);
    }

    #[test]
    fn a_staff_block_applies_by_name_for_its_map_load_unless_opted_out() {
        let mut blocks = MapBlocks::default();
        blocks.enter(Some(7), true);
        let here = roster(&[(2, "Padawan"), (5, "^1Troll"), (6, "Padawan")]);
        let list = [
            staff(5, "Troll", 7),
            staff(6, "Someone else", 7),
            staff(2, "Padawan", 6),
        ];
        // By name, colours aside; another map load's block is ignored.
        assert_eq!(blocks.resolve(&here, Some(0), &list), 1 << 5);
        assert_eq!(blocks.shared_slots(), 1 << 5);
        // A rename seen here keeps it.
        let renamed = roster(&[(2, "Padawan"), (5, "Innocent"), (6, "Padawan")]);
        assert_eq!(blocks.resolve(&renamed, Some(0), &list), 1 << 5);
        // A client that never saw the match does not follow the new name.
        let mut late = MapBlocks::default();
        late.enter(Some(7), true);
        assert_eq!(late.resolve(&renamed, Some(0), &list), 0);
        // Lifted here alone, until the player leaves.
        assert!(blocks.unblock(5));
        assert_eq!(blocks.resolve(&renamed, Some(0), &list), 0);
        let gone = roster(&[(2, "Padawan"), (6, "Padawan")]);
        assert_eq!(blocks.resolve(&gone, Some(0), &list), 0);
        assert_eq!(blocks.resolve(&here, Some(0), &list), 1 << 5);
        // Staff lifted it: gone.
        assert_eq!(blocks.resolve(&here, Some(0), &list[1..]), 0);
        // Opted out (`cl_staffBlocks 0`).
        blocks.enter(Some(7), false);
        assert_eq!(blocks.resolve(&here, Some(0), &list), 0);
    }

    #[test]
    fn clearing_lifts_own_and_staff_blocks_here() {
        let mut blocks = MapBlocks::default();
        blocks.enter(Some(7), true);
        let here = roster(&[(3, "A"), (5, "B")]);
        blocks.block(3, "A");
        let list = [staff(5, "B", 7)];
        assert_eq!(blocks.resolve(&here, None, &list), 0b101000);
        assert_eq!(blocks.clear(), 2);
        assert_eq!(blocks.resolve(&here, None, &list), 0);
    }
}
