//! `CG_VehicleClipCheck`, codemp/cgame/cg_predict.c:164-225 at OpenJK 4d0dfaf1.
use sjk_protocol::EntityState;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipActor {
    number: u16,
    kind: u8,
    class: u8,
    vehicle: u16,
}

impl ClipActor {
    pub(crate) fn from_entity(state: &EntityState) -> Self {
        Self {
            number: state.number(),
            kind: state.entity_type(),
            class: state.npc_class(),
            vehicle: state.vehicle_entity_num(),
        }
    }

    pub(crate) fn player(number: u16, vehicle: u16) -> Self {
        Self {
            number,
            kind: 1,
            class: 0,
            vehicle,
        }
    }

    fn actor(self) -> bool {
        matches!(self.kind, 1 | 13)
    }
    fn rider(self) -> bool {
        self.kind == 1 || self.kind == 13 && self.class != 53
    }

    pub(crate) fn collides(self, other: Self) -> bool {
        if other.number >= 1022
            || !self.actor()
            || self.vehicle == 0
            || !other.actor()
            || other.vehicle == 0
        {
            return true;
        }
        let parent = if self.rider() {
            self.vehicle
        } else {
            self.vehicle - 1
        };
        parent != other.number && !(other.rider() && other.vehicle == self.number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("prediction_vehicle_clip_fixtures.rs");

    fn actor([number, kind, class, vehicle]: [u16; 4]) -> ClipActor {
        ClipActor {
            number,
            kind: kind as u8,
            class: class as u8,
            vehicle,
        }
    }

    #[test]
    fn vehicle_clip_matches_openjk_emitted_matrix() {
        for &(ignored, hit, expected) in FIXTURES {
            assert_eq!(
                actor(ignored).collides(actor(hit)),
                expected,
                "ignored {ignored:?}, hit {hit:?}"
            );
        }
    }
}
