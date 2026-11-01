//! Editing a recorded run (`docs/client.md`, "Ghost editor"): keep the commands from a
//! first to a last one and drop the chat balloon (`BUTTON_TALK`) the console or chat left
//! in them.
//!
//! A replay walks onto the run's start and stands still there before it plays the first
//! command ([`super::approach_step`]), so a run cut in the middle of a movement begins
//! without the speed it needs and drifts. [`Cut`] therefore knows which commands a run
//! can begin on cleanly: those the player was standing still on the ground before.
//! Clearing the balloon never changes a movement: the client sends no movement keys
//! while a key catcher is open, so a command that carries the balloon moves nobody.

use super::{Frame, Run, START_STILL, Start, steer};
use sjk_game_jka::pmove_talk::BUTTON_TALK;

/// What the player was doing during one command, for the timeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Motion {
    /// Standing still on the ground.
    Still,
    /// Moving on the ground.
    Ground,
    /// In the air.
    Air,
    /// The chat balloon was up (the console or chat was open).
    Talk,
}

/// A stretch of commands doing the same [`Motion`]: `first..=last`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Span {
    pub(crate) motion: Motion,
    pub(crate) first: usize,
    pub(crate) last: usize,
}

/// A point of the run's path, for drawing it in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PathPoint {
    /// The feet, in world units.
    pub(crate) at: [f32; 3],
    /// The command it was taken at.
    pub(crate) index: u32,
    /// The chat bubble was up.
    pub(crate) talk: bool,
}

/// A run being cut: the run as recorded, where it can begin, and its timeline.
#[derive(Clone, Debug)]
pub(crate) struct Cut {
    pub(super) run: Run,
    /// The commands a replay can begin on cleanly, in order ([`clean_start`]).
    clean: Vec<u32>,
    pub(crate) spans: Vec<Span>,
}

/// Whether the player stood still on the ground before command `index`: a replay begun
/// there starts as the recording did. The first command always counts: it is the
/// recording's own start.
fn clean_start(run: &Run, index: usize) -> bool {
    let Some(before) = index.checked_sub(1).and_then(|i| run.frames.get(i)) else {
        return true;
    };
    before.ground
        && steer::horizontal(before.velocity) < START_STILL
        && before.velocity[2].abs() < START_STILL
}

fn talks(frame: &Frame) -> bool {
    frame.buttons & BUTTON_TALK != 0
}

/// Whether `frame` presses something or still moves: a key, a button the player pressed,
/// a generic action, speed, or the air.
fn moves(frame: &Frame) -> bool {
    frame.forward != 0
        || frame.right != 0
        || frame.up != 0
        || frame.buttons & !super::PASSIVE_BUTTONS != 0
        || frame.generic_command != 0
        || !frame.ground
        || steer::horizontal(frame.velocity) >= START_STILL
}

fn motion(frame: &Frame) -> Motion {
    if talks(frame) {
        Motion::Talk
    } else if !frame.ground {
        Motion::Air
    } else if steer::horizontal(frame.velocity) >= START_STILL {
        Motion::Ground
    } else {
        Motion::Still
    }
}

impl Cut {
    pub(super) fn new(run: Run) -> Self {
        let clean = (0..run.frames.len())
            .filter(|&index| clean_start(&run, index))
            .map(|index| index as u32)
            .collect();
        let mut spans: Vec<Span> = Vec::new();
        for (index, frame) in run.frames.iter().enumerate() {
            let motion = motion(frame);
            match spans.last_mut() {
                Some(span) if span.motion == motion => span.last = index,
                _ => spans.push(Span {
                    motion,
                    first: index,
                    last: index,
                }),
            }
        }
        Self { run, clean, spans }
    }

    /// The number of commands; never zero (a loaded run has one at least).
    pub(crate) fn len(&self) -> usize {
        self.run.frames.len()
    }

    /// Command `index`'s time, in milliseconds from the run's first.
    pub(crate) fn time(&self, index: usize) -> i32 {
        self.run.frames.get(index).map_or(0, |frame| frame.time)
    }

    /// The run's length in milliseconds.
    pub(crate) fn duration(&self) -> i32 {
        self.time(self.len() - 1)
    }

    /// The last command at or before `time` (milliseconds).
    pub(crate) fn index_at(&self, time: i32) -> usize {
        self.run
            .frames
            .partition_point(|frame| frame.time <= time)
            .saturating_sub(1)
    }

    pub(crate) fn is_clean(&self, index: usize) -> bool {
        self.clean.binary_search(&(index as u32)).is_ok()
    }

    /// The clean start nearest `index`, the earlier one on a tie.
    pub(crate) fn nearest_clean(&self, index: usize) -> usize {
        let at = self
            .clean
            .partition_point(|&clean| (clean as usize) < index);
        let after = self.clean.get(at).map(|&clean| clean as usize);
        let before = at
            .checked_sub(1)
            .and_then(|i| self.clean.get(i))
            .map(|&clean| clean as usize);
        match (before, after) {
            (Some(before), Some(after)) if after - index < index - before => after,
            (Some(before), _) => before,
            (None, Some(after)) => after,
            (None, None) => 0,
        }
    }

    /// The first clean start from `index` on (`forward`) or back from it, if any.
    pub(crate) fn clean_from(&self, index: usize, forward: bool) -> Option<usize> {
        let at = self
            .clean
            .partition_point(|&clean| (clean as usize) < index);
        // Back from `index`: `index` itself when it is clean, else the one before.
        let on_it = self
            .clean
            .get(at)
            .is_some_and(|&clean| clean as usize == index);
        let found = if forward || on_it {
            self.clean.get(at)
        } else {
            at.checked_sub(1).and_then(|i| self.clean.get(i))
        };
        found.map(|&clean| clean as usize)
    }

    /// The clean starts, for the timeline's strip: stretches `first..=last`.
    pub(crate) fn clean_stretches(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut index = 0;
        std::iter::from_fn(move || {
            let first = *self.clean.get(index)? as usize;
            let mut last = first;
            index += 1;
            while self
                .clean
                .get(index)
                .is_some_and(|&next| next as usize == last + 1)
            {
                last += 1;
                index += 1;
            }
            Some((first, last))
        })
    }

    /// Where the player first moves from `from` on: the command to begin on so that the
    /// standing still before it (the console closing, the wait) is cut. `None` when
    /// nothing moves after `from`.
    pub(crate) fn first_move(&self, from: usize) -> Option<usize> {
        let found = self.run.frames.get(from..)?.iter().position(moves)? + from;
        // The command where the keys go down was stood still before: a clean start.
        self.clean_from(found, false)
    }

    /// The command the player stops on, after the last that moves, up to `until`.
    pub(crate) fn last_move(&self, until: usize) -> Option<usize> {
        let frames = self.run.frames.get(..=until)?;
        let found = frames.iter().rposition(moves)?;
        Some((found + 1).min(until))
    }

    /// The chat balloon's stretches among commands `first..=last`.
    pub(crate) fn talk_stretches(&self, first: usize, last: usize) -> usize {
        self.spans
            .iter()
            .filter(|span| span.motion == Motion::Talk && span.last >= first && span.first <= last)
            .count()
    }

    /// The run as it begins on command `index`: its start, as a replay walks to it.
    pub(super) fn start_at(&self, index: usize) -> Start {
        match index.checked_sub(1).and_then(|i| self.run.frames.get(i)) {
            Some(before) => Start {
                origin: before.origin,
                view: before.view,
                velocity: before.velocity,
                weapon: before.weapon,
            },
            None => self.run.start,
        }
    }

    /// The player's position after command `index` (before the first: the start).
    pub(crate) fn origin(&self, index: usize) -> [f32; 3] {
        self.run
            .frames
            .get(index)
            .map_or(self.run.start.origin, |frame| frame.origin)
    }

    pub(crate) fn map(&self) -> &str {
        &self.run.map
    }

    /// The timeline in `columns` equal slices of time: what each slice shows (the chat
    /// bubble over the air over moving over standing), and whether a clean start lies in
    /// it. Read once per run, so a frame draws a bounded number of shapes.
    pub(crate) fn columns(&self, columns: usize) -> Vec<(Motion, bool)> {
        let duration = self.duration().max(1) as i64;
        let column_of = |index: usize| {
            ((i64::from(self.time(index)) * columns as i64 / duration) as usize).min(columns - 1)
        };
        let rank = |motion: Motion| match motion {
            Motion::Still => 0,
            Motion::Ground => 1,
            Motion::Air => 2,
            Motion::Talk => 3,
        };
        let mut out = vec![(Motion::Still, false); columns];
        for span in &self.spans {
            let (from, to) = (column_of(span.first), column_of(span.last));
            for column in &mut out[from..=to] {
                if rank(span.motion) > rank(column.0) {
                    column.0 = span.motion;
                }
            }
        }
        for (first, last) in self.clean_stretches() {
            for column in &mut out[column_of(first)..=column_of(last)] {
                column.1 = true;
            }
        }
        out
    }

    /// The run's path for drawing: at most `most` points of the feet (`FEET` under the
    /// origin), each `spacing` units or more from the one before, with the command it was
    /// taken at and whether the chat bubble was up. The first and last commands are always
    /// in it.
    pub(crate) fn path(&self, spacing: f32, most: usize) -> Vec<PathPoint> {
        const FEET: f32 = 24.0;
        let point = |index: usize, frame: &Frame| PathPoint {
            at: [frame.origin[0], frame.origin[1], frame.origin[2] - FEET],
            index: index as u32,
            talk: talks(frame),
        };
        let mut spacing = spacing.max(1.0);
        // The bubble's turns are always kept: past a map's size, spacing cannot thin the
        // path any more, so it is cut short instead.
        for _ in 0..24 {
            let mut points: Vec<PathPoint> = Vec::new();
            let last = self.len() - 1;
            for (index, frame) in self.run.frames.iter().enumerate() {
                let far = points.last().is_none_or(|kept| {
                    let d = [
                        frame.origin[0] - kept.at[0],
                        frame.origin[1] - kept.at[1],
                        frame.origin[2] - FEET - kept.at[2],
                    ];
                    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] >= spacing * spacing
                });
                let turned = points.last().is_some_and(|kept| kept.talk != talks(frame));
                if far || turned || index == last {
                    points.push(point(index, frame));
                }
            }
            if points.len() <= most || spacing > 65_536.0 {
                points.truncate(most);
                return points;
            }
            spacing *= 2.0;
        }
        Vec::new()
    }

    /// The run keeping commands `first..=last`, its clock begun again at the first and its
    /// start the state before it; without the chat balloon when `clear_talk`.
    pub(super) fn edited(&self, first: usize, last: usize, clear_talk: bool) -> Run {
        let last = last.min(self.len() - 1);
        let first = first.min(last);
        let zero = self.time(first);
        let frames = self.run.frames[first..=last]
            .iter()
            .map(|frame| Frame {
                time: frame.time - zero,
                buttons: if clear_talk {
                    frame.buttons & !BUTTON_TALK
                } else {
                    frame.buttons
                },
                ..*frame
            })
            .collect();
        Run {
            map: self.run.map.clone(),
            start: self.start_at(first),
            frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(time: i32, forward: i8, speed: f32, ground: bool, buttons: u16) -> Frame {
        Frame {
            time,
            view: [0, time, 0],
            forward,
            right: 0,
            up: 0,
            buttons,
            weapon: 3,
            force_selection: 0,
            inventory_selection: 255,
            generic_command: 0,
            origin: [time as f32, 0.0, 24.0],
            velocity: [speed, 0.0, 0.0],
            ground,
            confirmed: true,
        }
    }

    /// The console open for three commands (`ghost record` typed), standing for two, a run
    /// of three on the ground and two in the air, a stop, then the console again (`ghost
    /// stop`).
    fn recorded() -> Run {
        let talk = BUTTON_TALK;
        Run {
            map: "mp/ffa3".into(),
            start: Start {
                origin: [-8.0, 0.0, 24.0],
                view: [0, 7, 0],
                velocity: [0.0; 3],
                weapon: 3,
            },
            frames: vec![
                frame(0, 0, 0.0, true, talk),
                frame(8, 0, 0.0, true, talk),
                frame(16, 0, 0.0, true, talk),
                frame(24, 0, 0.0, true, 0),
                frame(32, 0, 0.0, true, 0),
                frame(40, 127, 120.0, true, 0),
                frame(48, 127, 250.0, true, 0),
                frame(56, 127, 250.0, false, 0),
                frame(64, 127, 250.0, false, 0),
                frame(72, 0, 80.0, true, 0),
                frame(80, 0, 0.0, true, 0),
                frame(88, 0, 0.0, true, 0),
                frame(96, 0, 0.0, true, talk),
                frame(104, 0, 0.0, true, talk),
            ],
        }
    }

    #[test]
    fn a_run_begins_cleanly_only_where_the_player_stood_still_before() {
        let cut = Cut::new(recorded());
        let clean: Vec<usize> = (0..cut.len()).filter(|&i| cut.is_clean(i)).collect();
        assert_eq!(clean, [0, 1, 2, 3, 4, 5, 11, 12, 13]);
        assert_eq!(cut.nearest_clean(7), 5);
        assert_eq!(cut.nearest_clean(9), 11);
        assert_eq!(cut.clean_from(6, true), Some(11));
        assert_eq!(cut.clean_from(6, false), Some(5));
        assert_eq!(cut.clean_from(5, false), Some(5));
        assert_eq!(cut.clean_from(14, true), None);
        let stretches: Vec<_> = cut.clean_stretches().collect();
        assert_eq!(stretches, [(0, 5), (11, 13)]);
    }

    #[test]
    fn the_timeline_tells_the_balloon_the_ground_and_the_air() {
        let cut = Cut::new(recorded());
        let motions: Vec<_> = cut
            .spans
            .iter()
            .map(|span| (span.motion, span.first, span.last))
            .collect();
        assert_eq!(
            motions,
            [
                (Motion::Talk, 0, 2),
                (Motion::Still, 3, 4),
                (Motion::Ground, 5, 6),
                (Motion::Air, 7, 8),
                (Motion::Ground, 9, 9),
                (Motion::Still, 10, 11),
                (Motion::Talk, 12, 13),
            ]
        );
        assert_eq!(cut.talk_stretches(0, 13), 2);
        assert_eq!(cut.talk_stretches(3, 11), 0);
        assert_eq!(cut.talk_stretches(2, 11), 1);
    }

    #[test]
    fn first_and_last_move_cut_the_waits_at_both_ends() {
        let cut = Cut::new(recorded());
        assert_eq!(cut.first_move(0), Some(5));
        assert_eq!(cut.last_move(13), Some(10));
        assert_eq!(cut.last_move(3), None);
        assert_eq!(cut.first_move(12), None);
    }

    #[test]
    fn an_edited_run_begins_at_zero_from_the_state_before_its_first_command() {
        let cut = Cut::new(recorded());
        let run = cut.edited(5, 10, true);
        assert_eq!(run.frames.len(), 6);
        assert_eq!(run.frames[0].time, 0);
        assert_eq!(run.frames[5].time, 40);
        assert_eq!(run.frames[0].forward, 127);
        assert_eq!(
            run.start,
            Start {
                origin: [32.0, 0.0, 24.0],
                view: [0, 32, 0],
                velocity: [0.0; 3],
                weapon: 3,
            }
        );
        // Kept from the first command, the start stays the recording's.
        let whole = cut.edited(0, 13, false);
        assert_eq!(whole.start, recorded().start);
        assert_eq!(whole.frames, recorded().frames);
        // The balloon goes, and nothing else changes.
        let cleared = cut.edited(0, 13, true);
        assert!(cleared.frames.iter().all(|frame| !talks(frame)));
        for (a, b) in cleared.frames.iter().zip(&recorded().frames) {
            assert_eq!(Frame { buttons: 0, ..*a }, Frame { buttons: 0, ..*b });
        }
        // It survives its text.
        let back = Run::from_text(&run.to_text()).expect("parse");
        assert_eq!(back.frames, run.frames);
        assert_eq!(back.start, run.start);
    }

    #[test]
    fn the_columns_keep_the_loudest_motion_and_the_clean_starts() {
        let cut = Cut::new(recorded());
        // 104 ms in 13 columns of 8 ms: one command each, the last two share the last.
        let columns = cut.columns(13);
        assert_eq!(columns.len(), 13);
        assert_eq!(columns[0], (Motion::Talk, true));
        assert_eq!(columns[3], (Motion::Still, true));
        assert_eq!(columns[7], (Motion::Air, false));
        assert_eq!(columns[12], (Motion::Talk, true));
        // Fewer columns than commands: the bubble wins its column.
        assert_eq!(cut.columns(2)[0].0, Motion::Talk);
    }

    #[test]
    fn the_path_thins_out_but_keeps_its_ends_and_the_bubble_turns() {
        let cut = Cut::new(recorded());
        // Commands 8 units apart: every other is kept at 16, plus the bubble's edges.
        let path = cut.path(16.0, 100);
        let kept: Vec<u32> = path.iter().map(|point| point.index).collect();
        assert_eq!(kept, [0, 2, 3, 5, 7, 9, 11, 12, 13]);
        assert_eq!(path[0].at, [0.0, 0.0, 0.0]);
        assert!(path[0].talk && !path[2].talk && path[7].talk);
        // Too many points: the spacing doubles until they fit.
        let few = cut.path(1.0, 5);
        assert!(few.len() <= 5);
        assert_eq!(few.first().map(|point| point.index), Some(0));
        assert_eq!(few.last().map(|point| point.index), Some(13));
    }

    #[test]
    fn times_find_their_commands() {
        let cut = Cut::new(recorded());
        assert_eq!(cut.duration(), 104);
        assert_eq!(cut.index_at(0), 0);
        assert_eq!(cut.index_at(43), 5);
        assert_eq!(cut.index_at(500), 13);
        assert_eq!(cut.index_at(-5), 0);
    }
}
