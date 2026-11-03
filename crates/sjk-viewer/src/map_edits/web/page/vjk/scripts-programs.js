// From VisionJK's web admin page (codemp/game/webadmin/page/js/scripts-programs.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
/* ------------------------------------------------------------------ */
/* the shader scripts of the game                                      */
/* ------------------------------------------------------------------ */

// The view of the administration draws the map the way the game does: every surface with the stages its shader
// script names (images, blending, moving texture coordinates, colours that pulse), in the order of the game,
// under the sky of the map and in its fog. What the engine makes of a script is in rd-vanilla/tr_shader.cpp and
// tr_shade.cpp, the rules here are taken from there. Not done: glow, sprites on surfaces, portals.
//
// The editor keeps the plain view (an image times the light), which needs nothing but the map.

const whiteTex = makeTexture(1, 1, new Uint8Array([255, 255, 255, 255]), { mip: false });

// R_FogFactor in tr_image.cpp: how thick the fog of a brush is, 0 to 1, from how far the line of sight runs (s)
// and how deep under the surface of the fog a place is (t). The table of the engine is a square root in 256 steps.
function fogFactor(s, t) {
	s -= 1 / 512;
	if (s < 0 || t < 1 / 32) return 0;
	if (t < 31 / 32) s *= (t - 1 / 32) / (30 / 32);
	return Math.sqrt(Math.floor(Math.min(1, s * 8) * 255) / 255);
}

// the image the fog of a brush is drawn with (R_CreateFogImage): white, its alpha is the thickness
const fogTex = (() => {
	const W = 256, H = 32, data = new Uint8Array(W * H * 4).fill(255);
	for (let x = 0; x < W; x++) for (let y = 0; y < H; y++) data[(y * W + x) * 4 + 3] = 255 * fogFactor((x + 0.5) / W, (y + 0.5) / H);
	return makeTexture(W, H, data, { mip: false, clamp: true });
})();

// deformVertexes wave, bulge and move, as RB_DeformTessGeometry does them: a wave and a bulge move a vertex along
// its normal, by how far depends on where it is, and a move shifts all of them.
const DEFORM_GLSL = `uniform int uWaveFunc;	// 0 no wave, 1 sin, 2 triangle, 3 square, 4 sawtooth, 5 the same falling
uniform vec4 uWave;		// how fast it changes along the surface, its base, how far it goes, where it is in its wave
uniform vec4 uBulge;	// width, height, where it is, and 1 when it only pushes outwards. Height 0: none
uniform vec3 uMove;
float waveOf(int f, float x) {
	float r = fract(x);
	if (f == 2) return r < 0.25 ? r * 4.0 : r < 0.75 ? 2.0 - r * 4.0 : r * 4.0 - 4.0;
	if (f == 3) return r < 0.5 ? 1.0 : -1.0;
	if (f == 4) return r;
	if (f == 5) return 1.0 - r;
	return sin(r * 6.2831853);
}
vec3 deformed(vec3 p, vec3 n, vec2 st) {
	if (uWaveFunc != 0) p += n * (uWave.y + uWave.z * waveOf(uWaveFunc, (p.x + p.y + p.z) * uWave.x + uWave.w));
	if (uBulge.y != 0.0) p += n * (uBulge.w > 0.5 ? uBulge.y : sin(st.x * uBulge.x + uBulge.z) * uBulge.y);
	return p + uMove;
}`;

// What the vertex shader of the map and the one of a model share: the texture coordinates of the one or two
// images of a stage, its colour with what a vertex brings to it, and the fog of a brush.
const STAGE_VS_GLSL = `uniform int uTcGen;		// 0 the texture coordinates, 1 the ones of the lightmap, 2 the environment, 3 the fog
uniform vec3 uTcA; uniform vec3 uTcB;	// the tcMods of the stage as one matrix
uniform vec2 uTurb;		// tcMod turb: how far, and where in its wave it is
uniform int uTcGen2; uniform vec3 uTcA2; uniform vec3 uTcB2; uniform vec2 uTurb2;	// the same of its second image
uniform vec4 uColor;	// the colour of the stage that every vertex has
uniform int uRgbVertex;	// 2 times one minus the colour of the vertex, else when not 0 times that colour
uniform int uAlphaVertex;	// 1 times the alpha of the vertex, 2 times one minus it, 3 times the shine, 4 times how far the eye is
uniform int uFogAdj;	// what of the colour fades in a fog: 1 the colour, 2 colour and alpha, 3 the alpha
uniform float uPortalRange;
uniform vec4 uFogPlane;	// the side the fog of a brush ends at, looking into it (all 0: the fog has none)
uniform vec2 uFogSt;	// one over eight times its depth, and how far behind that side the eye is
out vec2 vTc; out vec2 vTc2; out vec4 vCol; out float vDepth;
// RB_CalcFogTexCoords: how far in front of the eye a vertex is (s), and what part of the way to it is in the
// fog (t), as the image of the fog wants them
vec2 fogTc(vec3 at, float depth) {
	float t = dot(at, uFogPlane.xyz) - uFogPlane.w;
	if (uFogSt.y < 0.0) t = t < 1.0 ? 0.03125 : 0.03125 + 0.9375 * t / (t - uFogSt.y);
	else t = t < 0.0 ? 0.03125 : 0.96875;
	return vec2(depth * uFogSt.x + 0.001953125, t);
}
// R_FogFactor, as fogFactor of the page
float fogFactor(vec2 st) {
	float s = st.x - 0.001953125;
	if (s < 0.0 || st.y < 0.03125) return 0.0;
	if (st.y < 0.96875) s *= (st.y - 0.03125) / 0.9375;
	return sqrt(floor(min(s * 8.0, 1.0) * 255.0) / 255.0);
}
vec2 stageTc(int gen, vec3 a, vec3 b, vec2 turb, vec2 st, vec2 lm, vec2 env, vec2 fog, vec3 at) {
	vec2 tc = gen == 1 ? lm : gen == 2 ? env : gen == 3 ? fog : st;
	if (turb.x != 0.0) {
		tc.x += sin(((at.x + at.z) * 0.0009765625 + turb.y) * 6.2831853) * turb.x;
		tc.y += sin((at.y * 0.0009765625 + turb.y) * 6.2831853) * turb.x;
	}
	return vec2(dot(a.xy, tc) + a.z, dot(b.xy, tc) + b.z);
}
// pos and norm as the model has them, eye in the same space. color: what the vertex brings to the colour of
// the stage. at: where the planes of the fog are measured (the map), turbAt: what a turb runs along. light:
// where the light that shines on it comes from.
void stageVertex(vec3 pos, vec3 norm, vec3 eye, vec2 st, vec2 lm, vec4 color, vec3 at, vec3 turbAt, vec3 light, float depth) {
	vec3 v = normalize(eye - pos);
	vec2 env = vec2(0.0), fog = vec2(0.0);
	if (uTcGen == 2 || uTcGen2 == 2) {
		vec3 r = norm * 2.0 * dot(norm, v) - v;
		env = vec2(0.5 + r.y * 0.5, 0.5 - r.z * 0.5);
	}
	if (uTcGen == 3 || uFogAdj != 0) fog = fogTc(at, depth);
	vTc = stageTc(uTcGen, uTcA, uTcB, uTurb, st, lm, env, fog, turbAt);
	vTc2 = stageTc(uTcGen2, uTcA2, uTcB2, uTurb2, st, lm, env, fog, turbAt);
	// ComputeColors: the engine too works the colour out for every vertex
	vec4 c = uColor;
	if (uRgbVertex == 2) c.rgb *= 1.0 - color.rgb; else if (uRgbVertex != 0) c.rgb *= color.rgb;
	if (uAlphaVertex == 1) c.a *= color.a;
	else if (uAlphaVertex == 2) c.a *= 1.0 - color.a;
	else if (uAlphaVertex == 3) {
		// alphaGen lightingSpecular, as RB_CalcSpecularAlpha: how much of a light is mirrored into the eye, to
		// the fourth power
		float s = max(dot(norm * 2.0 * dot(norm, light) - light, v), 0.0);
		c.a *= s * s * s * s;
	} else if (uAlphaVertex == 4) {
		// alphaGen portal: nothing at the eye, all of it from the range of the shader on
		c.a *= clamp(distance(eye, pos) / uPortalRange, 0.0, 1.0);
	}
	// a stage that blends fades out in a fog instead of being fogged (RB_CalcModulateColorsByFog and its kin)
	if (uFogAdj != 0) {
		float clear = 1.0 - fogFactor(fog);
		if (uFogAdj == 1) c.rgb *= clear; else if (uFogAdj == 2) c *= clear; else c.a *= clear;
	}
	vCol = c;
	vDepth = depth;
}`;

// The fragment shader of both: the colour of the stage times its one or two images, the alpha test, and the fog
// of the whole map, which the engine leaves to OpenGL (glFog in RB_IterateStagesGeneric).
const STAGE_FS = `#version 300 es
precision highp float;
in vec2 vTc; in vec2 vTc2; in vec4 vCol; in float vDepth;
uniform sampler2D uTex; uniform sampler2D uTex2;
uniform int uMulti;	// 0 one image, 1 two multiplied, 2 two added (the stages CollapseMultitexture made one of)
uniform vec4 uFog;	// what the colour fades to, and how: 0 no fog, 1 GL_EXP2, 2 GL_LINEAR
uniform vec2 uFogParm;	// GL_EXP2: the density. GL_LINEAR: where it ends and one over its length
uniform int uAlphaFunc;	// 1 GT0, 2 LT128, 3 GE128, 4 GE192
out vec4 o;
void main() {
	vec4 c = vCol * texture(uTex, vTc);
	if (uMulti == 1) c *= texture(uTex2, vTc2);
	else if (uMulti == 2) { vec4 t = texture(uTex2, vTc2); c = vec4(min(c.rgb + t.rgb, 1.0), c.a * t.a); }
	if (uAlphaFunc == 1 && c.a <= 0.0) discard;
	if (uAlphaFunc == 2 && c.a >= 0.5) discard;
	if (uAlphaFunc == 3 && c.a < 0.5) discard;
	if (uAlphaFunc == 4 && c.a < 0.75) discard;
	if (uFog.a > 0.5) {
		float d = uFogParm.x * vDepth;
		c.rgb = mix(uFog.rgb, c.rgb, uFog.a > 1.5 ? clamp((uFogParm.x - vDepth) * uFogParm.y, 0.0, 1.0) : exp(-d * d));
	}
	o = c;
}`;

const stageProg = compile(`#version 300 es
layout(location=0) in vec3 aPos; layout(location=1) in vec2 aSt; layout(location=2) in vec2 aLm; layout(location=3) in vec4 aCol; layout(location=4) in vec3 aNorm;
uniform mat4 uMvp; uniform vec3 uEye;
${DEFORM_GLSL}
${STAGE_VS_GLSL}
// deformVertexes autosprite (1) and autosprite2 (2): a quad that turns to the viewer. Its vertices come as the
// middle it turns around, and aLm says how far from it they are (see buildSprites).
uniform int uSprite; uniform vec3 uViewLeft; uniform vec3 uViewUp; uniform vec3 uViewFwd;
void main() {
	vec3 pos = aPos, norm = aNorm;
	if (uSprite == 1) { pos += uViewLeft * aLm.x + uViewUp * aLm.y; norm = -uViewFwd; }
	else if (uSprite == 2) { pos += normalize(cross(aNorm, uViewFwd)) * aLm.x; norm = -uViewFwd; }
	pos = deformed(pos, norm, aSt);
	gl_Position = uMvp * vec4(pos, 1.0);
	// for the shine of the map the engine has one light at a fixed place
	stageVertex(pos, norm, uEye, aSt, aLm, aCol, pos, pos, normalize(vec3(-960.0, 1980.0, 96.0) - pos), gl_Position.w);
}`, STAGE_FS);

// The same stages on a model: its vertices are placed by a matrix, it has no lightmap of its own, and what a
// vertex brings to the colour of a stage is the light the page gives its models, where a stage asks for the
// light of the place (rgbGen lightingDiffuse, and every model without a script). The environment is worked out
// in the space of the model, as the engine does it.
const modelStageProg = compile(`#version 300 es
layout(location=0) in vec3 aPos; layout(location=1) in vec3 aNorm; layout(location=2) in vec2 aUv;
uniform mat4 uMvp; uniform mat4 uModel; uniform vec3 uEyeLocal;
uniform vec3 uAmbient; uniform vec3 uDirected; uniform vec3 uLightDir;	// the light of the place, and where it comes from
${DEFORM_GLSL}
${STAGE_VS_GLSL}
void main() {
	vec3 pos = deformed(aPos, aNorm, aUv);
	vec4 world = uModel * vec4(pos, 1.0);
	gl_Position = uMvp * world;
	// as RB_CalcDiffuseColor: the ambient light, and the directed one by how much the surface faces it
	vec3 lit = min(uAmbient + max(dot(mat3(uModel) * aNorm, uLightDir), 0.0) * uDirected, vec3(1.0));
	// alphaGen lightingSpecular: for a model the light of the place is the one that is mirrored
	stageVertex(pos, aNorm, uEyeLocal, aUv, vec2(0.0), vec4(lit, 1.0), world.xyz, world.xyz, normalize(transpose(mat3(uModel)) * uLightDir), gl_Position.w);
}`, STAGE_FS);

// the second image of a stage is on the second unit
for (const prog of [stageProg, modelStageProg]) {
	gl.useProgram(prog.p);
	gl.uniform1i(prog.u.uTex2, 1);
}
