//! The sound sonar's arcs round the crosshair (`crate::sound_sonar`): one short arc a
//! sound, at its direction (ahead is up), thicker and brighter for a louder sound,
//! tinted by kind and fading out. Units follow the 480-line height times `cg_hudScale`.
use crate::sound_sonar::{CAPACITY, Kind, Markers};
use sjk_ui::{Color, DrawCommand, DrawList};
use std::f32::consts::FRAC_PI_2;

/// Draw commands at most: a shadow and an arc a sound.
pub(super) const COMMANDS: usize = CAPACITY * 2;
/// The ring's radius, an arc's length (radians) and its stroke, thin to thick.
const RADIUS: f32 = 52.0;
const SWEEP: f32 = 0.45;
const WIDTH: (f32, f32) = (2.0, 5.0);
const SHADOW: Color = Color::new(0.0, 0.0, 0.0, 0.5);

/// The tint of a kind of sound.
fn tint(kind: Kind) -> Color {
    match kind {
        Kind::Steps => Color::new(0.85, 0.92, 1.0, 1.0),
        Kind::Combat => Color::new(1.0, 0.45, 0.15, 1.0),
        Kind::Voice => Color::new(0.55, 1.0, 0.55, 1.0),
        Kind::Other => Color::new(1.0, 0.85, 0.25, 1.0),
    }
}

/// Draw `markers` round `centre` (pixels) in a `viewport` of physical pixels.
pub(super) fn emit(
    list: &mut DrawList,
    viewport: [f32; 2],
    centre: [f32; 2],
    user_scale: f32,
    markers: &Markers,
) {
    let unit = viewport[1] / 480.0 * user_scale.clamp(0.25, 2.0) / 0.7;
    for marker in markers.iter() {
        let strength = marker.strength.clamp(0.0, 1.0);
        let width = (WIDTH.0 + (WIDTH.1 - WIDTH.0) * strength) * unit;
        // Arcs start from +X, clockwise on screen; ahead is up.
        let start = marker.angle - FRAC_PI_2 - SWEEP / 2.0;
        let alpha = marker.alpha * (0.45 + 0.55 * strength);
        let mut shadow = SHADOW;
        shadow.a *= alpha;
        let mut color = tint(marker.kind);
        color.a *= alpha;
        let _ = list.push(DrawCommand::Arc {
            center: centre,
            radius: RADIUS * unit,
            width: width + 2.0 * unit,
            start,
            sweep: SWEEP,
            color: shadow,
            knockout: None,
        });
        let _ = list.push(DrawCommand::Arc {
            center: centre,
            radius: RADIUS * unit,
            width,
            start,
            sweep: SWEEP,
            color,
            knockout: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_sonar::Marker;

    #[test]
    fn an_arc_ahead_is_centred_on_top_and_follows_the_hud_scale() {
        let mut markers = Markers::default();
        markers.list[0] = Marker {
            angle: 0.0,
            strength: 1.0,
            alpha: 1.0,
            kind: Kind::Combat,
        };
        markers.count = 1;
        let radius = |scale: f32| {
            let mut list = DrawList::new(COMMANDS);
            emit(&mut list, [1920.0, 1080.0], [960.0, 540.0], scale, &markers);
            assert_eq!(list.commands().len(), 2);
            match list.commands()[1] {
                DrawCommand::Arc {
                    radius,
                    start,
                    sweep,
                    ..
                } => {
                    assert!((start + sweep / 2.0 + FRAC_PI_2).abs() < 1e-5);
                    radius
                }
                _ => panic!("an arc"),
            }
        };
        assert!((radius(1.4) - 2.0 * radius(0.7)).abs() < 1e-3);
    }
}
