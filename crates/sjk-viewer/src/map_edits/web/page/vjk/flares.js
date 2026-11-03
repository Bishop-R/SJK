// From VisionJK's web admin page (codemp/game/webadmin/page/js/flares.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
// ---- the flares of a map: lights that show as a glow where nothing is in front of them

// Jedi Academy draws a flare as a surface like any other (RB_SurfaceFlare and RB_TestZFlare in
// rd-vanilla/tr_surface.cpp, r_flares 1; the flares of Quake 3 that fade in and out are not in this engine). When
// its turn comes the engine looks at the depth of the pixel the flare is at: if what was drawn there is not more
// than 24 units in front of it, the flare is a quad that faces the viewer, 3 units off its surface, with the
// shader of the surface. Its radius is the number of alphaGen portal in that shader, or 30, smaller the nearer
// than 512 units it is but never under 5, and the colour of its vertices is a grey that says how much the
// surface faces the viewer (a shader with rgbGen vertex takes it). The colour the map file has for it is not used.
//
// The depth can not be read back from the picture of a page without waiting for the graphics card. So every flare
// that is on the screen asks with a point of its own, 24 units nearer than itself, whether anything would cover
// it (an occlusion query), and is drawn by the last answer that has come back, which is one or two frames old.

const FLARE_STRIDE = 24;	// a vertex: its place and its texture coordinates as floats, its colour as four bytes
const FLARE_NEAR = 24;		// what is that near in front of a flare does not hide it

const flareTestProg = compile(`#version 300 es
layout(location=0) in vec3 aPos;
uniform mat4 uMvp; uniform vec2 uHalfPixel;
void main() {
	gl_Position = uMvp * vec4(aPos, 1.0);
	// the engine reads the pixel its rounded place is the corner of: half a pixel further
	gl_Position.xy += uHalfPixel * gl_Position.w;
	gl_PointSize = 1.0;
}`,
`#version 300 es
precision mediump float;
out vec4 o;
void main() { o = vec4(1.0); }`);

// What the flares of a map are drawn with, made when its scripts are there: the flares of one shader and fog
// together, each with its place in the vertices, and for every flare the query that asks whether it is hidden.
// Only the flares of the map itself: no map of the game has one in a brush model that an entity moves.
function prepareFlares(w) {
	const groups = new Map(), list = w.flares.filter(f => !f.model);
	for (const f of list) {
		const name = w.shaders[f.shader].name, key = name + ':' + f.fog;
		let g = groups.get(key);
		if (!g) {
			// a flare is lit by its vertices (ParseFlare)
			// the radius of a flare is the range of alphaGen portal of its shader, 30 without one (RB_SurfaceFlare)
			const mat = materialFor(name, false);
			groups.set(key, g = { mat, radius: mat.portalRange || 30, fog: f.fog, flares: [], first: 0, count: 0 });
		}
		g.flares.push({ origin: f.origin, normal: f.normal, at: f.origin.map((v, i) => v + 3 * f.normal[i]), query: gl.createQuery(), asked: false, visible: false, drawn: false, test: -1 });
	}
	const order = [...groups.values()].filter(g => g.mat.stages.length).sort((a, b) => a.mat.sort - b.mat.sort || a.mat.order - b.mat.order);
	let n = 0;
	for (const g of order) { g.first = n; n += g.flares.length; }
	const data = new ArrayBuffer(Math.max(1, n) * 4 * FLARE_STRIDE), vao = gl.createVertexArray(), buf = gl.createBuffer();
	gl.bindVertexArray(vao);
	gl.bindBuffer(gl.ARRAY_BUFFER, buf);
	gl.bufferData(gl.ARRAY_BUFFER, data.byteLength, gl.DYNAMIC_DRAW);
	gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, FLARE_STRIDE, 0);
	// RB_AddQuadStamp gives a quad the same coordinates for the image and for the lightmap
	gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 2, gl.FLOAT, false, FLARE_STRIDE, 12);
	gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 2, gl.FLOAT, false, FLARE_STRIDE, 12);
	gl.enableVertexAttribArray(3); gl.vertexAttribPointer(3, 4, gl.UNSIGNED_BYTE, true, FLARE_STRIDE, 20);
	// the two triangles of RB_AddQuadStamp: 0 1 3 and 3 1 2
	gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, gl.createBuffer());
	gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, Uint16Array.from({ length: n * 6 }, (_, i) => (i / 6 | 0) * 4 + [0, 1, 3, 3, 1, 2][i % 6]), gl.STATIC_DRAW);
	const testVao = gl.createVertexArray(), testBuf = gl.createBuffer(), tests = new Float32Array(Math.max(1, n) * 3);
	gl.bindVertexArray(testVao);
	gl.bindBuffer(gl.ARRAY_BUFFER, testBuf);
	gl.bufferData(gl.ARRAY_BUFFER, tests.byteLength, gl.DYNAMIC_DRAW);
	gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
	gl.bindVertexArray(null);
	// seen and drawn: how many flares were on the screen and how many of them were drawn in the last frame
	return { groups: order, vao, buf, floats: new Float32Array(data), bytes: new Uint8Array(data), testVao, testBuf, tests, total: n, seen: 0, drawn: 0 };
}

function drawFlares(now) {
	const w = world;
	if (!w || !w.flares.length || scripts.state !== 2 || !viewProj) return;
	const fd = w.flareDraw || (w.flareDraw = prepareFlares(w));
	if (!fd.total) return;
	const m = viewProj, axes = cameraAxes(), F = fd.floats, B = fd.bytes, T = fd.tests;
	let asking = 0, n = 0;
	fd.seen = fd.drawn = 0;
	for (const g of fd.groups) {
		g.count = 0;
		for (const f of g.flares) {
			// the answer to what an earlier frame asked
			if (f.asked && gl.getQueryParameter(f.query, gl.QUERY_RESULT_AVAILABLE)) { f.visible = !!gl.getQueryParameter(f.query, gl.QUERY_RESULT); f.asked = false; }
			f.test = -1; f.drawn = false;
			// off the screen: no flare (RB_TestZFlare), and when it comes back it is asked for anew
			const x = f.origin[0], y = f.origin[1], z = f.origin[2], cw = m[3] * x + m[7] * y + m[11] * z + m[15];
			const cx = m[0] * x + m[4] * y + m[8] * z + m[12], cy = m[1] * x + m[5] * y + m[9] * z + m[13], cz = m[2] * x + m[6] * y + m[10] * z + m[14];
			if (cx >= cw || cx <= -cw || cy >= cw || cy <= -cw || cz >= cw || cz <= -cw) { f.visible = false; continue; }
			fd.seen++;
			if (cw <= FLARE_NEAR + 4) {
				// nearer to the eye than anything can be in front of it by 24 units
				f.visible = true;
			} else if (!f.asked) {
				// the point that asks: on the line from the eye to the flare, 24 units less deep
				const k = 1 - FLARE_NEAR / cw;
				T[asking * 3] = eye[0] + (x - eye[0]) * k; T[asking * 3 + 1] = eye[1] + (y - eye[1]) * k; T[asking * 3 + 2] = eye[2] + (z - eye[2]) * k;
				f.test = asking++;
			}
			if (!f.visible) continue;
			// RB_SurfaceFlare: the quad
			const dx = f.at[0] - eye[0], dy = f.at[1] - eye[1], dz = f.at[2] - eye[2];
			const dist = Math.hypot(dx, dy, dz) || 1;
			const c = Math.min(255, Math.abs(dx * f.normal[0] + dy * f.normal[1] + dz * f.normal[2]) / dist * 255) | 0;
			const r = Math.max(5, dist < 512 ? g.radius * dist / 512 : g.radius);
			for (let i = 0; i < 4; i++, n++) {
				// left and up, right and up, right and down, left and down
				const l = i === 0 || i === 3 ? r : -r, u = i < 2 ? r : -r, o = n * 6, b = n * FLARE_STRIDE + 20;
				F[o] = f.at[0] + axes.left[0] * l + axes.up[0] * u; F[o + 1] = f.at[1] + axes.left[1] * l + axes.up[1] * u; F[o + 2] = f.at[2] + axes.left[2] * l + axes.up[2] * u;
				F[o + 3] = i === 0 || i === 3 ? 0 : 1; F[o + 4] = i < 2 ? 0 : 1;
				B[b] = B[b + 1] = B[b + 2] = c; B[b + 3] = 255;
			}
			g.count++; f.drawn = true;
		}
		// the quads of a group lie one after the other from its first place on
		n = (g.first + g.flares.length) * 4;
	}

	gl.enable(gl.DEPTH_TEST);
	gl.disable(gl.BLEND);
	gl.disable(gl.CULL_FACE);
	if (asking) {
		gl.useProgram(flareTestProg.p);
		gl.uniformMatrix4fv(flareTestProg.u.uMvp, false, m);
		gl.uniform2f(flareTestProg.u.uHalfPixel, 1 / canvas.width, 1 / canvas.height);
		gl.bindVertexArray(fd.testVao);
		gl.bindBuffer(gl.ARRAY_BUFFER, fd.testBuf);
		gl.bufferSubData(gl.ARRAY_BUFFER, 0, T, 0, asking * 3);
		// nothing is drawn: only whether the point would be
		gl.colorMask(false, false, false, false);
		gl.depthMask(false);
		gl.depthFunc(gl.LESS);
		for (const g of fd.groups) {
			for (const f of g.flares) {
				if (f.test < 0) continue;
				gl.beginQuery(gl.ANY_SAMPLES_PASSED, f.query);
				gl.drawArrays(gl.POINTS, f.test, 1);
				gl.endQuery(gl.ANY_SAMPLES_PASSED);
				f.asked = true;
			}
		}
	}

	const t = now / 1000, fogged = fogShown();
	gl.useProgram(stageProg.p);
	gl.uniform1i(stageProg.u.uTex, 0);
	gl.uniform3fv(stageProg.u.uEye, eye);
	gl.uniformMatrix4fv(stageProg.u.uMvp, false, m);
	gl.uniform1i(stageProg.u.uSprite, 0);
	gl.activeTexture(gl.TEXTURE0);
	// the alpha of the canvas stays 1 whatever a stage blends
	gl.colorMask(true, true, true, false);
	// the normal of a quad looks at the viewer
	gl.vertexAttrib3f(4, -axes.fwd[0], -axes.fwd[1], -axes.fwd[2]);
	gl.bindVertexArray(fd.vao);
	gl.bindBuffer(gl.ARRAY_BUFFER, fd.buf);
	for (const g of fd.groups) {
		if (!g.count) continue;
		fd.drawn += g.count;
		gl.bufferSubData(gl.ARRAY_BUFFER, g.first * 4 * FLARE_STRIDE, B, g.first * 4 * FLARE_STRIDE, g.count * 4 * FLARE_STRIDE);
		setCull(viewCulls() ? g.mat.cull : 2);
		setDeform(stageProg, g.mat, t);
		if (g.mat.polygonOffset) { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(-1, -2); }
		// The fog the flares are in, for drawStage, which fades what blends in a fog (adjustColorsForFog): that is
		// what makes a flare weaker with the distance on a map with fog. A stage with depthFunc disable is drawn
		// over everything by drawStage itself.
		const fog = (fogged && g.fog >= 0 && w.fogInfo[g.fog]) || null;
		for (const st of g.mat.stages) drawStage(st, t, null, fd.vao, g.count * 6, gl.UNSIGNED_SHORT, g.first * 12, stageProg, fog);
		if (g.mat.polygonOffset) gl.disable(gl.POLYGON_OFFSET_FILL);
	}
	setDeform(stageProg, NO_DEFORM, t);
	gl.bindVertexArray(null);
	gl.enable(gl.DEPTH_TEST);
	gl.disable(gl.CULL_FACE);
	gl.disable(gl.BLEND);
	gl.depthMask(true);
	gl.depthFunc(gl.LESS);
	gl.colorMask(true, true, true, true);
}
