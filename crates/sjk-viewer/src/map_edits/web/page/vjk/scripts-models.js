// From VisionJK's web admin page (codemp/game/webadmin/page/js/scripts-models.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
// The light an entity gets at a place, as the engine works it out (R_SetupEntityLightingGrid and
// R_SetupEntityLighting in tr_light.cpp): the eight samples of the light grid around the place, weighed by how
// near they are, without the ones inside walls. The ambient light times r_ambientScale (0.6) with a little added
// for everything, the directed light, and the direction it comes from. All three in out, colours from 0 to 1.
const lightNow = { ambient: [0, 0, 0], directed: [0, 0, 0], dir: [0, 0, 1] };
// a map without a grid: the same light everywhere, from where the sun of the engine is
const LIGHT_NO_GRID = { ambient: (150 + 32) / 255, directed: 150 / 255, dir: [0.433, 0.289, 0.854] };

function gridLight(pos, out = lightNow) {
	const g = world && world.grid, a = [0, 0, 0], d = [0, 0, 0], n = [0, 0, 0];
	if (!g || g.data.length < 30 || !g.array.length) {
		out.ambient.fill(LIGHT_NO_GRID.ambient);
		out.directed.fill(LIGHT_NO_GRID.directed);
		for (let c = 0; c < 3; c++) out.dir[c] = LIGHT_NO_GRID.dir[c];
		dlightsOn(pos, out);
		return out;
	}
	const at = [0, 0, 0], frac = [0, 0, 0];
	for (let i = 0; i < 3; i++) {
		const v = (pos[i] - g.origin[i]) / g.size[i];
		at[i] = Math.floor(v);
		frac[i] = v - at[i];
		if (at[i] < 0) at[i] = 0; else if (at[i] >= g.bounds[i] - 1) at[i] = g.bounds[i] - 1;
	}
	const step = [1, g.bounds[0], g.bounds[0] * g.bounds[1]], start = at[0] + at[1] * step[1] + at[2] * step[2];
	let total = 0;
	for (let i = 0; i < 8; i++) {
		let factor = 1, cellAt = start;
		for (let j = 0; j < 3; j++) {
			if (i & (1 << j)) { factor *= frac[j]; cellAt += step[j]; } else factor *= 1 - frac[j];
		}
		if (cellAt < 0 || cellAt >= g.array.length) continue;
		// a sample: 4 x 3 bytes of ambient light, 4 x 3 of directed, 4 styles, 2 for the direction
		const o = g.array[cellAt] * 30;
		if (o + 30 > g.data.length || g.data[o + 24] === 255) continue;	// none there, or one inside a wall
		total += factor;
		for (let j = 0; j < 4 && g.data[o + 24 + j] !== 255; j++) {
			for (let c = 0; c < 3; c++) { a[c] += factor * g.data[o + j * 3 + c]; d[c] += factor * g.data[o + 12 + j * 3 + c]; }
		}
		const lat = g.data[o + 29] / 256 * Math.PI * 2, lng = g.data[o + 28] / 256 * Math.PI * 2;
		n[0] += factor * Math.cos(lat) * Math.sin(lng);
		n[1] += factor * Math.sin(lat) * Math.sin(lng);
		n[2] += factor * Math.cos(lng);
	}
	const scale = total > 0 && total < 0.99 ? 1 / total : 1, len = Math.hypot(n[0], n[1], n[2]);
	for (let c = 0; c < 3; c++) {
		out.ambient[c] = Math.min(255, a[c] * scale * 0.6 + 32) / 255;
		out.directed[c] = d[c] * scale / 255;
		out.dir[c] = len ? n[c] / len : 0;
	}
	// what shines in this frame is added (js/dlights.js)
	dlightsOn(pos, out);
	return out;
}

// The surfaces of a model with the stages of their shaders, in the view of the administration. place: where the
// model stands. What is solid is drawn at once. A surface that is see-through (a sort after the banners) waits
// in lateModels until everything solid is there, and is then drawn in the order of its sort with the see-through
// surfaces of the map, as the game sorts all of them together. Without that, what is drawn later behind such a
// surface would wipe it out.
const NO_DEFORM = { deforms: [] };
const MINLIGHT_ADD = [16 / 255, 96 / 255, 150 / 255];
const lateModels = [];
const LATE_LAST = 1e9;		// the sort of what is drawn after everything else
let deferLate = false;		// true from the start of a frame of the administration until its late part is drawn
let lateSorted = true;

function modelPlace(place, light) {
	const u = modelStageProg.u, at = M4.transform(M4.invert(place), eye);
	gl.uniformMatrix4fv(u.uModel, false, place);
	gl.uniform3f(u.uEyeLocal, at[0], at[1], at[2]);
	gl.uniform3fv(u.uAmbient, light.ambient);
	gl.uniform3fv(u.uDirected, light.directed);
	gl.uniform3fv(u.uLightDir, light.dir);
}

function modelPassBegin() {
	gl.useProgram(modelStageProg.p);
	gl.uniformMatrix4fv(modelStageProg.u.uMvp, false, viewProj);
	gl.uniform1i(modelStageProg.u.uTex, 0);
	gl.colorMask(true, true, true, false);
	stageBegin(modelStageProg, fogShown() && world ? world.globalFog : null);
}

function modelPassEnd() {
	stEntity[0] = stEntity[1] = stEntity[2] = stEntity[3] = 1;
	stTint = stForceAlpha = false;
	gl.disable(gl.CULL_FACE);
	gl.disable(gl.BLEND);
	gl.depthMask(true);
	gl.depthFunc(gl.LESS);
	gl.colorMask(true, true, true, true);
}

// How far the vertices of a model reach from its origin, read back from its meshes once. Until they have any
// (a model whose bones were never set) it is asked again.
function meshRadius(meshes) {
	if (meshes.radius) return meshes.radius;
	let far = 0;
	for (const mesh of meshes) {
		gl.bindVertexArray(mesh.vao);
		const stride = gl.getVertexAttrib(0, gl.VERTEX_ATTRIB_ARRAY_STRIDE) || 12, first = gl.getVertexAttribOffset(0, gl.VERTEX_ATTRIB_ARRAY_POINTER);
		gl.bindBuffer(gl.ARRAY_BUFFER, gl.getVertexAttrib(0, gl.VERTEX_ATTRIB_ARRAY_BUFFER_BINDING));
		const pos = new Float32Array(gl.getBufferParameter(gl.ARRAY_BUFFER, gl.BUFFER_SIZE) >> 2);
		gl.getBufferSubData(gl.ARRAY_BUFFER, 0, pos);
		for (let i = first >> 2; i + 2 < pos.length; i += stride >> 2) far = Math.max(far, pos[i] * pos[i] + pos[i + 1] * pos[i + 1] + pos[i + 2] * pos[i + 2]);
	}
	gl.bindVertexArray(null);
	return meshes.radius = Math.sqrt(far);
}

// fog: the fog the model is in, as the map has it for its surfaces
function drawModelMesh(mesh, mat, t, fog) {
	setCull(mat.cull);
	setDeform(modelStageProg, mat, t);
	if (mat.polygonOffset) { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(-1, -2); }
	for (const st of mat.stages) drawStage(st, t, null, mesh.vao, mesh.count, gl.UNSIGNED_INT, 0, modelStageProg, fog);
	// the fog of a brush, drawn onto it as onto the map (RB_FogPass)
	if (fog && !fog.global && mat.fogPass) drawStage(fog.pass[mat.fogPass - 1], t, null, mesh.vao, mesh.count, gl.UNSIGNED_INT, 0, modelStageProg, fog);
	if (mat.polygonOffset) gl.disable(gl.POLYGON_OFFSET_FILL);
}

// ent: what the game says of the entity that is drawn and the renderer uses (its refEntity_t), all of it optional.
// rgba is its colour (shaderRGBA, 0 to 1) for the stages that ask for it, white without. radius is the ball
// around it that says which fog it is in, as far as its vertices reach without. shader is the name of a shader
// that is drawn instead of the one of every surface (customShader). tint (RF_RGB_TINT): every stage takes the
// colour of the entity. forceAlpha (RF_FORCE_ENT_ALPHA): every stage is blended by the alpha of the entity, and
// the entity is drawn after everything else of the scene. minLight (RF_MINLIGHT): more ambient light.
function drawModelScripted(meshes, place, ent) {
	const t = frameNow / 1000, at = [place[12], place[13], place[14]];
	modelPassBegin();
	const light = gridLight(at);
	// RF_MINLIGHT: a fixed bluish add to the ambient light (R_SetupEntityLighting: 16 96 150 of 255), which the
	// game gives items that float and the weapon in front of the camera
	if (ent && ent.minLight) for (let c = 0; c < 3; c++) light.ambient[c] = Math.min(1, light.ambient[c] + MINLIGHT_ADD[c]);
	modelPlace(place, light);
	// The fog of the entity is the fog of all its surfaces: the first of the map its ball reaches into (fogAt).
	// A place that scales the model scales the ball.
	const fog = fogShown() && world && world.fogInfo.length
		? fogAt(at, ent && ent.radius !== undefined ? ent.radius : meshRadius(meshes) * Math.hypot(place[0], place[1], place[2])) : null;
	const rgba = (ent && ent.rgba) || null, tint = !!(ent && ent.tint), forceAlpha = !!(ent && ent.forceAlpha);
	const custom = ent && ent.shader ? materialFor(ent.shader, 'model') : null;
	if (rgba) for (let i = 0; i < 4; i++) stEntity[i] = rgba[i];
	stTint = tint; stForceAlpha = forceAlpha;
	let kept = null;
	for (const mesh of meshes) {
		const mat = custom || mesh.mat || (mesh.mat = materialFor(mesh.shaderName || '', 'model'));
		if (mat.nodraw) continue;
		if (deferLate && (forceAlpha || mat.sort > SC_SORT_BANNER)) {
			// the place and the light are the ones of now: the next model changes both
			if (!kept) kept = { place: place.slice(), light: { ambient: light.ambient.slice(), directed: light.directed.slice(), dir: light.dir.slice() }, fog, rgba: rgba && rgba.slice(), tint, forceAlpha };
			// what is blended by the alpha of its entity comes after everything else (the post renders of the engine)
			lateModels.push({ mesh, mat, at: kept, order: lateModels.length, sort: forceAlpha ? LATE_LAST : mat.sort });
			lateSorted = false;
			continue;
		}
		drawModelMesh(mesh, mat, t, fog);
	}
	modelPassEnd();
}

// Draws what waits in lateModels and comes before a surface of the map with that sort and order of its shader
// (prepareScripted): a lower sort, or the same sort and a shader that was registered before. The shaders of
// models are registered after the ones of the map as far as the page knows, so among the same sort the map comes
// first. True when something was drawn: the program in use and the state of the blending are then those of a model.
function drawLateModels(sort, order = Infinity) {
	if (!lateModels.length) return false;
	if (!lateSorted) lateModels.sort((a, b) => a.sort - b.sort || a.mat.order - b.mat.order || a.order - b.order);
	lateSorted = true;
	let n = 0;
	while (n < lateModels.length && (lateModels[n].sort < sort || (lateModels[n].sort === sort && lateModels[n].mat.order < order))) n++;
	if (!n) return false;
	const t = frameNow / 1000;
	modelPassBegin();
	let at = null;
	for (const m of lateModels.splice(0, n)) {
		if (m.at !== at) {
			at = m.at;
			modelPlace(at.place, at.light);
			for (let i = 0; i < 4; i++) stEntity[i] = at.rgba ? at.rgba[i] : 1;
			stTint = at.tint; stForceAlpha = at.forceAlpha;
		}
		drawModelMesh(m.mesh, m.mat, t, at.fog);
	}
	modelPassEnd();
	return true;
}
// the administration draws models with their scripts once those are read
const modelsScripted = () => gameLook() && scripts.state === 2;
