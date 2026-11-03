// From VisionJK's web admin page (codemp/game/webadmin/page/js/pick.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
// Whether the map is drawn as the game draws it, with its shader scripts: the administration, and the editor with
// "As in the game" chosen, which it is to begin with. The other choices of the editor are its plain view.
const gameLook = () => appMode !== 'editor' || $('dbgLight').value === '3';
// Whether the map is drawn with one side of its surfaces only, the side the game draws: the administration once
// it has the scripts, the editor when "Open" is set, in its plain view and in the look of the game alike. Without
// it the editor draws both sides, so that a room is closed from outside. A click on the map goes through what
// is not drawn.
const viewCulls = () => cullSides && !!world && (appMode === 'editor' ? $('dbgCull').checked : !!world.scripted);

// the debug view of the editor: the triangles of the map as lines
const lineProg = compile(`#version 300 es
layout(location=0) in vec3 aPos; uniform mat4 uMvp;
void main() { gl_Position = uMvp * vec4(aPos, 1.0); }`,
`#version 300 es
precision mediump float;
uniform vec4 uColor; out vec4 o;
void main() { o = uColor; }`);

// points for that program, as lines or triangles: { vao, buf, count }
function pointsVao(data) {
	const vao = gl.createVertexArray(), buf = gl.createBuffer();
	gl.bindVertexArray(vao);
	gl.bindBuffer(gl.ARRAY_BUFFER, buf);
	gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
	gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
	gl.bindVertexArray(null);
	return { vao, buf, count: data.length / 3 };
}

function wireOf(b) {
	if (b.wire) return b.wire;
	const t = b.tri, lines = new Uint32Array(t.length * 2);
	for (let i = 0, o = 0; i < t.length; i += 3) {
		lines[o++] = t[i]; lines[o++] = t[i + 1];
		lines[o++] = t[i + 1]; lines[o++] = t[i + 2];
		lines[o++] = t[i + 2]; lines[o++] = t[i];
	}
	const vao = gl.createVertexArray(), ib = gl.createBuffer();
	gl.bindVertexArray(vao);
	gl.bindBuffer(gl.ARRAY_BUFFER, b.posBuf);
	gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
	gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
	gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, lines, gl.STATIC_DRAW);
	gl.bindVertexArray(null);
	return b.wire = { vao, count: lines.length };
}

function drawWorldWire(now) {
	gl.useProgram(lineProg.p);
	gl.uniformMatrix4fv(lineProg.u.uMvp, false, viewProj);
	gl.uniform4fv(lineProg.u.uColor, [0.35, 0.95, 0.55, 1]);
	gl.depthFunc(gl.LEQUAL);
	let placed = null;
	for (const b of world.batches) {
		const wire = wireOf(b);
		gl.bindVertexArray(wire.vao);
		for (const place of moverPlaces(b.model, now)) {
			if (place !== placed) gl.uniformMatrix4fv(lineProg.u.uMvp, false, (placed = place) ? M4.mul(viewProj, place) : viewProj);
			gl.drawElements(gl.LINES, wire.count, gl.UNSIGNED_INT, 0);
		}
	}
	gl.depthFunc(gl.LESS);
}

// Ray against the map triangles, returns the hit point or null. oneSided: only a triangle that is hit on its
// front counts (the side its corners run clockwise on), as the view shows it. A brush model that is an entity
// (a door, a lift) is hit where that entity is, and not at all when no entity shows it: for that the ray is
// taken to where the map file has the model.
let pickTri = -1;		// the triangle the last ray hit, as its number in world.pick
function pickGround(origin, dir, oneSided = viewCulls()) {
	pickTri = -1;
	if (!world) return null;
	const now = performance.now();
	let best = Infinity;
	for (const [model, from, to] of world.pick.runs) {
		for (const place of moverPlaces(model, now)) {
			if (!place) { best = pickRun(origin, dir, from, to, oneSided, best); continue; }
			const back = M4.invert(place), o = M4.transform(back, origin), e = M4.transform(back, [origin[0] + dir[0], origin[1] + dir[1], origin[2] + dir[2]]);
			best = pickRun(o, [e[0] - o[0], e[1] - o[1], e[2] - o[2]], from, to, oneSided, best);
		}
	}
	return best === Infinity ? null : [origin[0] + dir[0] * best, origin[1] + dir[1] * best, origin[2] + dir[2] * best];
}

// the nearest hit of the ray among the triangles from to, when it is nearer than best
function pickRun(origin, dir, from, to, oneSided, best) {
	const p = world.pick.pos, idx = world.pick.idx;
	for (let i = from; i < to; i += 3) {
		const a = idx[i] * 3, b = idx[i + 1] * 3, c = idx[i + 2] * 3;
		const e1x = p[b] - p[a], e1y = p[b + 1] - p[a + 1], e1z = p[b + 2] - p[a + 2];
		const e2x = p[c] - p[a], e2y = p[c + 1] - p[a + 1], e2z = p[c + 2] - p[a + 2];
		const hx = dir[1] * e2z - dir[2] * e2y, hy = dir[2] * e2x - dir[0] * e2z, hz = dir[0] * e2y - dir[1] * e2x;
		const det = e1x * hx + e1y * hy + e1z * hz;
		if (oneSided ? det > -1e-7 : Math.abs(det) < 1e-7) continue;
		const f = 1 / det, sx = origin[0] - p[a], sy = origin[1] - p[a + 1], sz = origin[2] - p[a + 2];
		const u = f * (sx * hx + sy * hy + sz * hz);
		if (u < 0 || u > 1) continue;
		const qx = sy * e1z - sz * e1y, qy = sz * e1x - sx * e1z, qz = sx * e1y - sy * e1x;
		const v = f * (dir[0] * qx + dir[1] * qy + dir[2] * qz);
		if (v < 0 || u + v > 1) continue;
		const t = f * (e2x * qx + e2y * qy + e2z * qz);
		if (t > 1 && t < best) { best = t; pickTri = i / 3; }
	}
	return best;
}

