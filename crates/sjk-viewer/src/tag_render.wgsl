// Tags on the world (tag_render.rs): each picture at its own size, with its finish shaded
// per pixel as the cards' are (sjk_ui::shine, ui_shapes.wgsl): Holo's rainbow foil, its
// light following the viewer's angle, and Solar's corona burning round the picture's own
// outline (a signed distance to its edge, `outline`).

struct Camera {
    view_projection: mat4x4<f32>,
    position: vec3<f32>,
    _padding: f32,
    forward: vec3<f32>,
    view_flags: f32,
}

struct Globals {
    time: f32,
    _a: f32,
    _b: f32,
    _c: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(1) @binding(0)
var tag_sampler: sampler;
@group(1) @binding(1)
var<uniform> globals: Globals;
@group(2) @binding(0)
var picture: texture_2d<f32>;
@group(2) @binding(1)
var outline: texture_2d<f32>;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) right: vec3<f32>,
    @location(3) down: vec3<f32>,
    // The finish (0 Normal, 1 Holo, 2 Solar) and a seed of the tag's own.
    @location(4) params: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) right: vec3<f32>,
    @location(3) down: vec3<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
}

@vertex
fn vertex_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = camera.view_projection * vec4(input.position, 1.0);
    out.uv = input.uv;
    out.world = input.position;
    out.right = input.right;
    out.down = input.down;
    out.params = input.params;
    return out;
}

// The finishes' scales are the cards': pixels across a picture this wide.
const SIZE: f32 = 512.0;
// The outline's reach either way, a share of the picture's edge (tag_render.rs
// OUTLINE_REACH).
const OUTLINE_REACH: f32 = 0.25;

fn wrap_angle(angle: f32) -> f32 {
    return angle - 6.2831855 * floor((angle + 3.1415927) / 6.2831855);
}

// PCG hash of a lattice point, in [0, 1] (sjk_ui::shine::hash).
fn shine_hash(cell: vec2<i32>) -> f32 {
    var v = bitcast<u32>(cell.x) * 747796405u + 2891336453u;
    v = v ^ (bitcast<u32>(cell.y) * 2654435761u);
    v = v * 747796405u + 2891336453u;
    let word = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    return f32((word >> 22u) ^ word) / 4294967295.0;
}

fn shine_noise(p: vec2<f32>) -> f32 {
    let c = floor(p);
    let f = p - c;
    let u = f * f * (3.0 - 2.0 * f);
    let cell = vec2<i32>(c);
    let a = shine_hash(cell);
    let b = shine_hash(cell + vec2(1, 0));
    let d = shine_hash(cell + vec2(0, 1));
    let e = shine_hash(cell + vec2(1, 1));
    let top = a + (b - a) * u.x;
    let bottom = d + (e - d) * u.x;
    return top + (bottom - top) * u.y;
}

// Straight-alpha `top` at `alpha` over straight-alpha `under`.
fn shine_over(under: vec4<f32>, top: vec3<f32>, alpha_in: f32) -> vec4<f32> {
    let alpha = clamp(alpha_in, 0.0, 1.0);
    let out = alpha + under.a * (1.0 - alpha);
    if out <= 0.0 {
        return vec4(0.0);
    }
    return vec4((top * alpha + under.rgb * under.a * (1.0 - alpha)) / out, out);
}

const SUN_CORE: vec3<f32> = vec3(1.0, 0.97, 0.86);
const SUN_INNER: vec3<f32> = vec3(1.0, 0.72, 0.24);
const SUN_RIM_HOT: vec3<f32> = vec3(1.0, 0.42, 0.05);
const SUN_RIM_COOL: vec3<f32> = vec3(1.0, 0.2, 0.02);
const SUN_MOTE: vec3<f32> = vec3(1.0, 0.82, 0.42);

// Holo: a pearl veil, a rainbow foil whose hue runs across the slant through the light,
// strongest about it, and a white glint (sjk_ui::shine::holo). Stronger and with more
// bands than a card's: a wall is seen from further than a card held, and a tag is often
// one flat bright colour, which a faint foil barely tints.
fn shine_holo(p: vec2<f32>, t: f32, light: vec2<f32>) -> vec4<f32> {
    let u = p.x / SIZE;
    let v = p.y / SIZE;
    let across = (p.x - light.x * SIZE) + 0.84 * (p.y - light.y * SIZE);
    let k = across / (0.68 * SIZE);
    let pearl = mix(vec3(0.75, 0.85, 1.0), vec3(1.0, 0.7, 0.95), u);
    var out = vec4(pearl, 0.08 + 0.04 * u);
    let hue = k * 1.5 + 0.5 + 0.07 * sin(u * 5.0 + v * 2.0 + t * 0.6);
    var rainbow = 0.5 + 0.5 * cos(6.2831855 * (vec3(hue) - vec3(0.0, 1.0 / 3.0, 2.0 / 3.0)));
    rainbow = rainbow + (vec3(1.0) - rainbow) * 0.25;
    let envelope = exp(-k * k * 1.6);
    out = shine_over(out, rainbow, 0.16 + 0.4 * envelope);
    let line = across / (0.03 * SIZE);
    let halo = across / (0.12 * SIZE);
    let glint = exp(-line * line) * 0.32 + exp(-halo * halo) * 0.08;
    return shine_over(out, vec3(1.0), glint);
}

// Solar: the Sun blade's corona burning along the edge, `inside` pixels in from it
// (sjk_ui::shine::solar); a picture's edge is its outline, so the corona follows its shape.
fn shine_solar(p: vec2<f32>, inside: f32, t: f32, light: vec2<f32>) -> vec4<f32> {
    let reach = SIZE * 0.06;
    let u = p.x / SIZE;
    let v = p.y / SIZE;
    var out = vec4(SUN_INNER, 0.06 * (1.0 - v));
    let across = (p.x - light.x * SIZE) + 0.7 * (p.y - light.y * SIZE);
    let wide = across / (0.16 * SIZE);
    let thin = across / (0.025 * SIZE);
    out = shine_over(out, SUN_INNER, exp(-wide * wide) * 0.07);
    out = shine_over(out, SUN_CORE, exp(-thin * thin) * 0.13);
    if inside > reach * 3.2 {
        return out;
    }
    let angle = atan2(v - 0.5, u - 0.5);
    var flare = 0.0;
    for (var index = 0; index < 3; index++) {
        let i = f32(index);
        let direction = select(1.0, -1.0, index == 1);
        let centre = direction * t * (0.32 + 0.11 * i) + i * 2.1;
        let off = wrap_angle(angle - centre) / 0.3;
        let swell = 0.55 + 0.45 * sin(t * 1.3 + i * 1.7);
        flare += exp(-off * off) * swell;
    }
    let q = p / reach;
    var grain = 0.62 * shine_noise(vec2(q.x * 0.55 + t * 0.35, q.y * 0.55 - t * 0.9))
        + 0.38 * shine_noise(vec2(q.x * 1.3 - t * 0.6, q.y * 1.3 - t * 1.6 + 17.0));
    grain = smoothstep(0.15, 0.85, grain);
    let ridge = 1.0 - abs(2.0 * shine_noise(vec2(q.x * 0.8 + 31.0, q.y * 0.8 - t * 1.2)) - 1.0);
    let tongue = ridge * ridge * ridge;
    let height = reach * (0.6 + 0.7 * grain + 0.9 * tongue + 0.8 * flare);
    let depth = max(inside, 0.0) / height;
    let shimmer = 1.0 + 0.045 * sin(t * 9.0 + u * 4.4) + 0.03 * sin(t * 15.7 - v * 8.2);
    if depth < 1.0 {
        let body = pow(1.0 - depth, 1.6);
        let bright = min(0.45 + 0.4 * grain + 0.25 * tongue + 0.6 * flare, 1.0);
        let rim = mix(SUN_RIM_HOT, SUN_RIM_COOL, clamp(1.0 - grain, 0.0, 1.0));
        var colour = mix(SUN_INNER, rim, smoothstep(0.15, 0.85, depth));
        let c = max(inside, 0.0) / (reach * 0.16);
        let core = exp(-c * c) * (0.6 + 0.4 * min(flare, 1.0));
        colour = mix(colour, SUN_CORE, core);
        out = shine_over(out, colour, min(body * bright * shimmer, 0.95));
    }
    let cell = reach * 0.9;
    let rise = vec2(p.x / cell, (p.y + t * reach * 1.1) / cell);
    let base = floor(rise);
    let at = vec2<i32>(base);
    let seed = shine_hash(at);
    if seed > 0.86 {
        let mx = base.x + 0.2 + 0.6 * shine_hash(at + vec2(7, 0));
        let my = base.y + 0.2 + 0.6 * shine_hash(at + vec2(0, 7));
        let dx = (rise.x - mx) * cell;
        let dy = (rise.y - my) * cell * 0.6;
        let dot_size = reach * 0.09;
        let spot = (dx * dx + dy * dy) / (dot_size * dot_size);
        let twinkle = 0.5 + 0.5 * sin(t * 3.0 + seed * 40.0);
        let fade = 1.0 - smoothstep(reach * 1.2, reach * 3.0, inside);
        out = shine_over(out, SUN_MOTE, exp(-spot) * twinkle * fade * 0.9);
    }
    return out;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let base = textureSample(picture, tag_sampler, input.uv);
    // Sampled in uniform control flow, before the finish branches.
    let edge = textureSample(outline, tag_sampler, input.uv).r;
    let finish = u32(input.params.x + 0.5);
    var out = base;
    if finish != 0u {
        let t = globals.time + input.params.y * 37.0;
        // The light crosses where the viewer's angle puts it: straight on, the middle;
        // walking past, it slides across, as light on foil does; and it drifts a little.
        let view = normalize(input.world - camera.position);
        let drift = vec2(sin(t * 0.37), cos(t * 0.29)) * 0.06;
        let light = vec2(0.5 + 1.1 * dot(view, input.right), 0.5 + 1.1 * dot(view, input.down))
            + drift;
        let p = input.uv * SIZE;
        if finish == 1u {
            // The foil lies on the picture only, with a glitter of points that catch the
            // light as the viewer moves.
            let foil = shine_holo(p, t, light);
            var rgb = mix(base.rgb, foil.rgb, clamp(foil.a, 0.0, 1.0));
            let grid = p / 7.0;
            let cell = vec2<i32>(floor(grid));
            let seed = shine_hash(cell);
            let centre = floor(grid) + vec2(0.3 + 0.4 * shine_hash(cell + vec2(3, 0)),
                0.3 + 0.4 * shine_hash(cell + vec2(0, 3)));
            let spot = length(grid - centre);
            let angle = dot(view, input.right) * 23.0 + dot(view, input.down) * 17.0;
            let catch_light = pow(max(sin(angle + seed * 61.0 + t * 0.8), 0.0), 12.0);
            let glitter = select(0.0, catch_light * exp(-spot * spot * 18.0), seed > 0.82);
            rgb = mix(rgb, vec3(1.0), clamp(glitter * 0.9, 0.0, 1.0));
            out = vec4(rgb, base.a);
        } else {
            // Pixels in from the outline (out of it below 0).
            let inside = (edge - 0.5) * 2.0 * OUTLINE_REACH * SIZE;
            let sun = shine_solar(p, max(inside, 0.0), t, light);
            if inside >= 0.0 {
                out = shine_over(base, sun.rgb, sun.a);
            } else {
                // Outside, the corona's glow fades within a few of its reaches, and to
                // nothing before the decal's own edge, so no square shows round it.
                let border = min(min(input.uv.x, 1.0 - input.uv.x), min(input.uv.y, 1.0 - input.uv.y));
                let fade = smoothstep(0.0, 0.06, border);
                let halo = sun.a * exp(inside / (SIZE * 0.06 * 0.6)) * 0.85 * fade;
                out = shine_over(base, sun.rgb, halo);
            }
        }
    }
    if out.a <= 0.003 {
        discard;
    }
    return out;
}
