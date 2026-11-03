//! The Monitor toy's marks on the crosshair (`crate::monitor`): while a throw is held, a
//! ring round the crosshair fills clockwise from the top, yellow turning orange, and
//! glows white once full; when one of the player's monitors strikes someone, four short
//! strokes stand out diagonally from the crosshair and fade, spreading a little (red for
//! a hard blow), as shooters mark a hit.
//!
//! Units follow the 480-line height, as the hack bar's (`hack_bar.rs`), centred on the
//! crosshair as it is drawn.
use sjk_ui::{Color, DrawCommand, DrawList};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

/// What the toy shows: how far a throw held has charged (0 to 1), and how far the hit
/// marker is through its time (0 to 1) and whether its blow was hard.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MonitorMarks {
    pub(crate) charge: Option<f32>,
    pub(crate) hit: Option<(f32, bool)>,
}

/// The ring's radius and stroke, its track's colour, and its fill's colours empty,
/// nearly full and full.
const RING_RADIUS: f32 = 17.0;
const RING_WIDTH: f32 = 2.5;
const TRACK: Color = Color::new(0.0, 0.0, 0.0, 0.45);
const LOW: Color = Color::new(1.0, 0.9, 0.2, 0.9);
const HIGH: Color = Color::new(1.0, 0.45, 0.1, 0.95);
const FULL: Color = Color::new(1.0, 1.0, 1.0, 1.0);
/// The hit marker's strokes: from this far out from the centre, this long, this thick
/// (spreading this much more as it fades), and its colours.
const MARK_INNER: f32 = 7.0;
const MARK_LENGTH: f32 = 7.0;
const MARK_WIDTH: f32 = 2.0;
const MARK_SPREAD: f32 = 4.0;
const MARK: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const MARK_HARD: Color = Color::new(1.0, 0.2, 0.15, 1.0);
const SHADOW: Color = Color::new(0.0, 0.0, 0.0, 0.6);

/// Draw `marks` round the crosshair's `centre`, in pixels, in a `viewport` of physical
/// pixels.
pub(super) fn emit(list: &mut DrawList, viewport: [f32; 2], centre: [f32; 2], marks: MonitorMarks) {
    let unit = viewport[1] / 480.0;
    if let Some(charge) = marks.charge {
        let charge = charge.clamp(0.0, 1.0);
        let radius = RING_RADIUS * unit;
        let _ = list.push(DrawCommand::Arc {
            center: centre,
            radius,
            width: (RING_WIDTH + 1.5) * unit,
            start: 0.0,
            sweep: TAU,
            color: TRACK,
            knockout: None,
        });
        if charge > 0.0 {
            let _ = list.push(DrawCommand::Arc {
                center: centre,
                radius,
                width: RING_WIDTH * unit,
                start: -FRAC_PI_2,
                sweep: TAU * charge,
                color: if charge >= 1.0 {
                    FULL
                } else {
                    mix(LOW, HIGH, charge)
                },
                knockout: None,
            });
        }
    }
    if let Some((age, hard)) = marks.hit {
        let age = age.clamp(0.0, 1.0);
        let fade = 1.0 - age * age;
        let color = if hard { MARK_HARD } else { MARK };
        let inner = (MARK_INNER + MARK_SPREAD * age) * unit;
        let outer = inner + MARK_LENGTH * unit;
        for corner in 0..4 {
            let angle = FRAC_PI_4 + FRAC_PI_2 * corner as f32;
            let (sin, cos) = angle.sin_cos();
            let from = [centre[0] + cos * inner, centre[1] + sin * inner];
            let to = [centre[0] + cos * outer, centre[1] + sin * outer];
            for (width, color) in [
                ((MARK_WIDTH + 1.5) * unit, with_alpha(SHADOW, fade)),
                (MARK_WIDTH * unit, with_alpha(color, fade)),
            ] {
                let _ = list.push(DrawCommand::Line {
                    from,
                    to,
                    width,
                    color,
                });
            }
        }
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(marks: MonitorMarks) -> DrawList {
        let mut list = DrawList::new(32);
        emit(&mut list, [1920.0, 1080.0], [960.0, 540.0], marks);
        list
    }

    /// Through the whole HUD: the ring stands round the crosshair in the middle of the
    /// screen (it once took the crosshair's middle, a share of the screen, for pixels,
    /// and stood in the top left corner).
    #[test]
    fn the_hud_draws_the_ring_round_the_crosshair_in_the_middle() {
        use crate::hud::{HudLook, HudOverlay, HudVisibility};
        let font = crate::text::load_modern(1.0, None).unwrap().font;
        let mut hud = HudOverlay::new();
        hud.set_monitor_marks(MonitorMarks {
            charge: Some(0.5),
            hit: Some((0.0, false)),
        });
        let visibility = HudVisibility {
            hud: true,
            elements: true,
            status: false,
            vehicle: false,
            weapon: false,
            selection: false,
            crosshair: true,
            crosshair_names: false,
            timer: false,
            lagometer: false,
            ground_hud: false,
            menu_hud: false,
        };
        let _ = hud.layout(&font, HudLook::Radial, [1920.0, 1080.0], 1.0, visibility, 0);
        let mut centres = 0;
        for command in hud.draw_list().commands() {
            match *command {
                DrawCommand::Arc { center, radius, .. } if radius == RING_RADIUS * 2.25 => {
                    assert_eq!(center, [960.0, 540.0]);
                    centres += 1;
                }
                DrawCommand::Line { from, .. } => {
                    assert!((from[0] - 960.0).abs() < 60.0 && (from[1] - 540.0).abs() < 60.0);
                    centres += 1;
                }
                _ => {}
            }
        }
        assert_eq!(
            centres,
            2 + 8,
            "the ring's two arcs and the marker's eight strokes"
        );
    }

    #[test]
    fn nothing_is_drawn_without_a_charge_or_a_hit() {
        assert!(draw(MonitorMarks::default()).is_empty());
    }

    #[test]
    fn the_ring_fills_from_the_top_as_the_throw_charges_and_turns_white_full() {
        let arcs = |marks| -> Vec<(f32, f32, Color)> {
            draw(marks)
                .commands()
                .iter()
                .filter_map(|command| match command {
                    DrawCommand::Arc {
                        start,
                        sweep,
                        color,
                        ..
                    } => Some((*start, *sweep, *color)),
                    _ => None,
                })
                .collect()
        };
        let charge = |charge| MonitorMarks {
            charge: Some(charge),
            hit: None,
        };
        // Pressed, nothing charged yet: the track alone.
        assert_eq!(arcs(charge(0.0)).len(), 1);
        let half = arcs(charge(0.5));
        assert_eq!(half.len(), 2);
        assert_eq!((half[1].0, half[1].1), (-FRAC_PI_2, TAU * 0.5));
        assert_ne!(half[1].2, FULL);
        assert_eq!(arcs(charge(1.0))[1].2, FULL);
    }

    #[test]
    fn a_hit_marks_four_diagonal_strokes_red_when_hard_and_fades() {
        let strokes = |age, hard| -> Vec<([f32; 2], [f32; 2], Color)> {
            draw(MonitorMarks {
                charge: None,
                hit: Some((age, hard)),
            })
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Line {
                    from, to, color, ..
                } => Some((*from, *to, *color)),
                _ => None,
            })
            .collect()
        };
        let fresh = strokes(0.0, false);
        // Four strokes, each over its shadow.
        assert_eq!(fresh.len(), 8);
        let (from, to, color) = fresh[1];
        assert_eq!(color, MARK);
        // Down and right, 7 units (2.25 pixels each) from the centre, diagonally.
        let reach = 7.0 * 2.25 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((from[0] - (960.0 + reach)).abs() < 0.01, "{from:?}");
        assert!((from[1] - (540.0 + reach)).abs() < 0.01, "{from:?}");
        assert!(to[0] > from[0] && to[1] > from[1]);
        assert_eq!(strokes(0.0, true)[1].2, MARK_HARD);
        let late = strokes(0.9, false);
        assert!(late[1].2.a < 0.3 && late[1].0[0] > from[0]);
    }
}
