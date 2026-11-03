// From VisionJK's web admin page (codemp/game/webadmin/page/js/world-ents.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: changed: each entity keeps its number in the lump, and the props a map edit hides are left out.
// ---- what the client game puts into a map by itself, from the entities of the map file (cg_spawn.c): the sky
// portal and the models of misc_model_static

// what a "gametype" key is matched against, by the number of the gametype
const GAMETYPE_NAMES = ['ffa', 'holocron', 'jedimaster', 'duel', 'powerduel', 'single', 'team', 'siege', 'ctf', 'cty'];

// The value of a key as CG_SpawnString finds it: the first key of that name, in large or small letters.
function spawnKey(e, key) {
	for (let i = 0; i < e.length; i += 2) if (e[i].toLowerCase() === key) return e[i + 1];
	return undefined;
}
// atof of a key: 0 without it
const spawnFloat = (e, key) => parseFloat(spawnKey(e, key)) || 0;

// CG_SpawnVector: the three numbers of a key into out. Without the key they are the default. A value that is not
// three numbers leaves 0 0 0 and counts as missing.
function spawnVector(e, key, out, def = 0) {
	const s = spawnKey(e, key);
	out[0] = out[1] = out[2] = def;
	if (s === undefined) return false;
	const v = s.trim().split(/\s+/);
	for (let i = 0; i < 3; i++) out[i] = parseFloat(v[i]);
	if (isFinite(out[0]) && isFinite(out[1]) && isFinite(out[2])) return true;
	out[0] = out[1] = out[2] = 0;
	return false;
}

// Whether an entity is there in a gametype: CG_ParseEntityFromSpawnVars, and G_SpawnGEntityFromSpawnVars of the
// server with the same rules.
function spawnsIn(e, gametype) {
	const flag = key => (parseInt(spawnKey(e, key), 10) || 0) !== 0;
	if (gametype === 5 && flag('notsingle')) return false;
	// from GT_TEAM on "notteam" counts, below it "notfree"
	if (flag(gametype >= 6 ? 'notteam' : 'notfree')) return false;
	const only = spawnKey(e, 'gametype'), name = GAMETYPE_NAMES[gametype];
	return only === undefined || !name || only.includes(name);
}

// The entities of the map file that the renderer and the client game read, taken from its text once for a map:
// each as its keys and values in the order of the file.
let mapEntsNow = null;
function mapEnts() {
	if (mapEntsNow && mapEntsNow.world === world) return mapEntsNow;
	const text = world.entityText || '', all = [], re = /"([^"]*)"|[{}]|[^\s{}"]+/g;
	for (let m, e = null, key = null; (m = re.exec(text));) {
		if (m[0] === '{') { e = []; key = null; }
		else if (m[0] === '}') { if (e) all.push(e); e = null; }
		else if (e) {
			const word = m[1] !== undefined ? m[1] : m[0];
			if (key === null) key = word; else { e.push(key, word); key = null; }
		}
	}
	// SJK: each entity's number in the lump, which a map edit names a prop by
	all.forEach((e, n) => { e.num = n; });
	// R_LoadEntities: the worldspawn says how far the map is drawn, 6000 without the key
	let distanceCull = 6000;
	const first = all[0] || [];
	for (let i = 0; i < first.length; i += 2) {
		if (first[i].toLowerCase() === 'distancecull' && isFinite(parseFloat(first[i + 1]))) distanceCull = parseFloat(first[i + 1]);
	}
	const of = name => all.filter(e => (spawnKey(e, 'classname') || '').toLowerCase() === name);
	// where a brush model that is an entity has its origin: its vertices are around that
	const origins = new Map();
	for (const e of all) {
		const model = spawnKey(e, 'model'), at = [0, 0, 0];
		if (model && model[0] === '*' && spawnVector(e, 'origin', at)) origins.set(+model.slice(1), at);
	}
	if (mapEntsNow) dropSplit(mapEntsNow);
	mapEntsNow = { world, distanceCull, origins, portals: of('misc_skyportal'), orients: of('misc_skyportal_orient'), statics: of('misc_model_static'),
		gametype: -1, portal: null, st: null, tree: undefined, zone: undefined, split: null, fog: null, fogTaken: false };
	return mapEntsNow;
}

// the same with what of them there is in the gametype that runs: the sky portal and the list of the static models
function mapEntsFor(gametype) {
	const me = mapEnts();
	if (me.gametype === gametype) return me;
	me.gametype = gametype;
	// The server says where the room of the portal is (SP_misc_skyportal in g_misc.c, the configstring
	// CS_SKYBOXORG): the origin of the last misc_skyportal it spawns, with two decimals. The "fov" and the fog
	// keys of the entity are in that string too; CG_DrawSkyBoxPortal reads them and then takes cg_fov and no fog
	// of its own. What the client game reads itself (cg_spawn.c): "onlyfoghere" of a misc_skyportal, and the
	// misc_skyportal_orient the eye of the room follows the viewer by, of which the last one counts.
	const spots = me.portals.filter(e => spawnsIn(e, gametype)), orient = me.orients.filter(e => spawnsIn(e, gametype)).pop();
	me.portal = null;
	if (spots.length) {
		const origin = [0, 0, 0], from = [0, 0, 0];
		spawnVector(spots[spots.length - 1], 'origin', origin);
		if (orient) spawnVector(orient, 'origin', from);
		me.portal = { origin: origin.map(v => +v.toFixed(2)), orient: orient ? from : null, scale: orient ? spawnFloat(orient, 'modelscale') : 0,
			onlyFogHere: spots.some(e => /^(qtrue|true|yes|1)$/i.test(spawnKey(e, 'onlyfoghere') || '')) };
	}
	me.zone = undefined;
	dropSplit(me);
	me.st = staticList(me.statics.filter(e => spawnsIn(e, gametype) && spawnKey(e, 'model') && !sjkHiddenProps.has(e.num)));
	return me;
}

/* the tree of the map file */

// The tree of a map file and which part of it sees which (the lumps of the nodes, the leafs and the visibility),
// as R_PointInLeaf, R_ClusterPVS and R_inPVS use them. A node is nine numbers (its plane, what is in front of it
// and behind it, its box), a leaf twelve (its cluster and its area, its box, its surfaces and brushes), and the
// visibility has a row of bits for every cluster: the clusters that are seen from it.
function bspTree(buffer, planes) {
	const dv = new DataView(buffer), lump = n => new Uint8Array(buffer, dv.getInt32(8 + n * 8, true), Math.max(0, dv.getInt32(12 + n * 8, true))).slice();
	const nodes = lump(3), leafs = lump(4), leafSurfaces = lump(5), leafBrushes = lump(6), vis = lump(16), head = new DataView(vis.buffer);
	// leafBrushes: the brushes of every leaf one after the other, for what asks what is at a point or along a line
	// (a leaf says the first of its brushes there and how many, the eleventh and twelfth of its twelve numbers)
	return { planes, nodes: new Int32Array(nodes.buffer, 0, nodes.length >> 2), leafs: new Int32Array(leafs.buffer, 0, leafs.length >> 2),
		leafBrushes: new Int32Array(leafBrushes.buffer, 0, leafBrushes.length >> 2),
		// and its surfaces the same way (the ninth and tenth number of a leaf), in the order the engine goes through them
		leafSurfaces: new Int32Array(leafSurfaces.buffer, 0, leafSurfaces.length >> 2),
		clusters: vis.length >= 8 ? head.getInt32(0, true) : 0, rowBytes: vis.length >= 8 ? head.getInt32(4, true) : 0, vis: vis.subarray(8) };
}

// the tree of the map that is shown, null until it is there
function mapTree(me) {
	if (me.tree === undefined) me.tree = world.tree || null;
	return me.tree;
}

// The cluster a point is in, -1 for none (in a wall, outside of the map). A point on a plane is behind it for
// the renderer (R_PointInLeaf) and in front of it for the collision code (CM_PointLeafnum_r, front).
function pointCluster(t, x, y, z, front) {
	const N = t.nodes, P = t.planes;
	if (!N.length) return -1;
	let n = 0;
	while (n >= 0) {
		const p = N[n * 9] * 4, d = P[p] * x + P[p + 1] * y + P[p + 2] * z - P[p + 3];
		n = N[n * 9 + (d > 0 || (front && d === 0) ? 1 : 2)];
	}
	return t.leafs[(-1 - n) * 12];
}

/* the sky portal */

// The part of the map the room of a portal is: the clusters that are seen from where the portal is and all that
// is seen from those, and the box around their leafs. Such a room is closed, so nothing else sees into it and
// it sees nothing else. null: the portal is in no cluster, the map has no visibility, or the portal is in the
// open and that part is all of the map. All of the map is drawn from there then.
function portalZone(tree, origin) {
	const start = pointCluster(tree, origin[0], origin[1], origin[2], false);
	if (start < 0 || start >= tree.clusters) return null;
	const flags = new Uint8Array(tree.clusters), todo = [start], mins = [Infinity, Infinity, Infinity], maxs = [-Infinity, -Infinity, -Infinity];
	flags[start] = 1;
	while (todo.length) {
		const row = todo.pop() * tree.rowBytes;
		for (let k = 0; k < tree.clusters; k++) {
			if (!flags[k] && (tree.vis[row + (k >> 3)] & (1 << (k & 7)))) { flags[k] = 1; todo.push(k); }
		}
	}
	let rest = false;
	for (let l = 0, L = tree.leafs; l < L.length; l += 12) {
		if (L[l] < 0) continue;
		if (!flags[L[l]]) { rest = true; continue; }
		for (let i = 0; i < 3; i++) { mins[i] = Math.min(mins[i], L[l + 2 + i]); maxs[i] = Math.max(maxs[i], L[l + 5 + i]); }
	}
	return rest ? { flags, mins, maxs } : null;
}

// Whether a triangle is in the room of a portal: pos has the vertices, a, b and c are where its corners begin
// in it, off is where the brush model it belongs to has its origin. The engine draws the surfaces of the leafs
// a view sees; the page has the surfaces of a shader in one piece and asks every triangle instead. In the box
// of the room, the cluster a little in front of the triangle says it, as it is seen from there, then the one
// behind it, and the box alone where it is in none.
const ROOM_STEPS = [1, 8, -1, -8], NO_ORIGIN = [0, 0, 0];
function inPortalRoom(me, pos, a, b, c, off) {
	const zone = me.zone, mins = zone.mins, maxs = zone.maxs;
	const x = (pos[a] + pos[b] + pos[c]) / 3 + off[0], y = (pos[a + 1] + pos[b + 1] + pos[c + 1]) / 3 + off[1], z = (pos[a + 2] + pos[b + 2] + pos[c + 2]) / 3 + off[2];
	if (x < mins[0] - 8 || y < mins[1] - 8 || z < mins[2] - 8 || x > maxs[0] + 8 || y > maxs[1] + 8 || z > maxs[2] + 8) return false;
	// the front of a triangle is the side its corners run clockwise on
	const ux = pos[b] - pos[a], uy = pos[b + 1] - pos[a + 1], uz = pos[b + 2] - pos[a + 2], vx = pos[c] - pos[a], vy = pos[c + 1] - pos[a + 1], vz = pos[c + 2] - pos[a + 2];
	const nx = vy * uz - vz * uy, ny = vz * ux - vx * uz, nz = vx * uy - vy * ux, l = Math.hypot(nx, ny, nz) || 1;
	let k = -1;
	for (let j = 0; j < 4 && k < 0; j++) {
		const d = ROOM_STEPS[j] / l;
		k = pointCluster(me.tree, x + nx * d, y + ny * d, z + nz * d, false);
	}
	return k < 0 || zone.flags[k] === 1;
}

// Where the surfaces of a shader are, for every shader and brush model of the map: 1 in the room, 2 outside of
// it, 3 both. The triangles of the map are kept for the click on it (world.pick), so this needs nothing from
// the graphics card, and only an entry whose shader is on both sides has to be read back and cut in two.
// null: that copy is not there, every entry is read back.
function roomShaders(me) {
	const pick = world.pick, surfs = world.surfs, shaders = world.shaders;
	if (!pick || !pick.surf || !surfs || !shaders) return null;
	const where = new Map(), pos = pick.pos, idx = pick.idx, n = idx.length / 3;
	let last = -1, key = '', off = NO_ORIGIN, sides = 0;
	for (let t = 0; t <= n; t++) {
		const k = t < n ? pick.surf[t] : -1;
		if (k !== last) {
			if (last >= 0) where.set(key, (where.get(key) || 0) | sides);
			if (k < 0) break;
			const surf = surfs[k];
			last = k;
			sides = 0;
			key = shaders[surf.shader].name + '|' + surf.model;
			off = (surf.model && me.origins.get(surf.model)) || NO_ORIGIN;
		}
		if (sides !== 3) sides |= inPortalRoom(me, pos, idx[t * 3] * 3, idx[t * 3 + 1] * 3, idx[t * 3 + 2] * 3, off) ? 1 : 2;
	}
	return where;
}

// a VAO with the vertices of another one and indexes of its own
function vaoWithIndexes(vao, idx, made) {
	const attrs = [];
	gl.bindVertexArray(vao);
	for (let loc = 0; loc < 8; loc++) {
		if (!gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_ENABLED)) continue;
		attrs.push([loc, gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_BUFFER_BINDING), gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_SIZE), gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_TYPE),
			gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_NORMALIZED), gl.getVertexAttrib(loc, gl.VERTEX_ATTRIB_ARRAY_STRIDE), gl.getVertexAttribOffset(loc, gl.VERTEX_ATTRIB_ARRAY_POINTER)]);
	}
	const out = gl.createVertexArray(), ib = gl.createBuffer();
	gl.bindVertexArray(out);
	for (const [loc, buf, size, type, normalized, stride, offset] of attrs) {
		gl.bindBuffer(gl.ARRAY_BUFFER, buf);
		gl.enableVertexAttribArray(loc);
		gl.vertexAttribPointer(loc, size, type, normalized, stride, offset);
	}
	gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
	gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
	gl.bindVertexArray(null);
	made.push(out, ib);
	return out;
}

// The two parts of a map are let go: what was made for them, and the map is drawn from its whole list again.
function dropSplit(me) {
	const s = me.split;
	if (!s) return;
	if (me.world.scripted === s.rest || me.world.scripted === s.room) me.world.scripted = s.src;
	for (const o of s.made) { if (o instanceof WebGLBuffer) gl.deleteBuffer(o); else gl.deleteVertexArray(o); }
	me.split = null;
}

// an entry for the view of the map: with "onlyfoghere" the map outside of the room has no fog (RDF_NOFOG, R_AddDrawSurf)
const outsideRoom = (me, e) => me.portal.onlyFogHere && e.fog ? Object.assign({}, e, { fog: null }) : e;

// One entry of the list the map is drawn from (prepareScripted), put to the room of the portal, to the rest of
// the map, or to both in two parts: what it draws is read back, its vertices and its indexes.
function splitEntry(me, s, e) {
	const off = (e.b.model && me.origins.get(e.b.model)) || NO_ORIGIN;
	gl.bindVertexArray(e.vao);
	gl.bindBuffer(gl.ARRAY_BUFFER, gl.getVertexAttrib(0, gl.VERTEX_ATTRIB_ARRAY_BUFFER_BINDING));
	const pos = new Float32Array(gl.getBufferParameter(gl.ARRAY_BUFFER, gl.BUFFER_SIZE) >> 2), idx = new Uint32Array(e.count);
	gl.getBufferSubData(gl.ARRAY_BUFFER, 0, pos);
	gl.getBufferSubData(gl.ELEMENT_ARRAY_BUFFER, 0, idx);
	gl.bindVertexArray(null);

	const n = idx.length / 3, inRoom = new Uint8Array(n);
	let count = 0;
	for (let t = 0; t < n; t++) {
		if (inPortalRoom(me, pos, idx[t * 3] * 3, idx[t * 3 + 1] * 3, idx[t * 3 + 2] * 3, off)) { inRoom[t] = 1; count++; }
	}
	if (!count) { s.rest.push(outsideRoom(me, e)); return; }
	if (count === n) { s.room.push(e); return; }
	const part = which => {
		const sub = new Uint32Array((which ? count : n - count) * 3);
		for (let t = 0, o = 0; t < n; t++) {
			if (inRoom[t] === which) { sub[o++] = idx[t * 3]; sub[o++] = idx[t * 3 + 1]; sub[o++] = idx[t * 3 + 2]; }
		}
		const vao = vaoWithIndexes(e.vao, sub, s.made);
		// the fog of a brush is drawn from the batch of the entry: that is this part then
		return Object.assign({}, e, { vao, count: sub.length, b: e.sprite ? e.b : Object.assign({}, e.b, { vao, count: sub.length }) });
	};
	s.room.push(part(1));
	s.rest.push(outsideRoom(me, part(0)));
}

// The list of the map in its two parts, for the list prepareScripted made: null while it is worked out, a few
// milliseconds of every frame, and while the tree of the map is not there.
function portalSplit(me, list) {
	const tree = mapTree(me);
	if (!tree) return null;
	if (me.zone === undefined) me.zone = portalZone(tree, me.portal.origin);
	let s = me.split;
	if (!s || s.src !== list) {
		dropSplit(me);
		s = me.split = { src: list, room: [], rest: [], next: 0, made: [], where: me.zone ? roomShaders(me) : null };
		if (!me.zone) { s.room = s.rest = list; s.next = list.length; }
	}
	for (const until = performance.now() + 4; s.next < list.length && performance.now() < until; s.next++) {
		// a shader the copy of the map does not know has nothing in the room
		const e = list[s.next], sides = s.where ? s.where.get(e.b.shaderName + '|' + e.b.model) || 2 : 3;
		if (sides === 2) s.rest.push(outsideRoom(me, e)); else if (sides === 1) s.room.push(e); else splitEntry(me, s, e);
	}
	return s.next < list.length ? null : s;
}

// The room of the sky portal, drawn from its own spot before the map (CG_DrawSkyBoxPortal in cg_view.c, and what
// the renderer does with RDF_SKYBOXPORTAL). true: what is behind the map is drawn, the map leaves its sky.
// skyPortalShown is the switch of the check script.
let skyPortalShown = true;
const portalVp = new Float32Array(16), portalEye = [0, 0, 0], portalMove = [0, 0, 0];
function drawSkyPortal(now) {
	const me = state ? mapEntsFor(state.gametype) : null, p = me && skyPortalShown ? me.portal : null;
	if (me) {
		// what the last frame left behind: the list of the map is the whole of it again, and its fog is back
		if (me.split && (world.scripted === me.split.rest || world.scripted === me.split.room)) world.scripted = me.split.src;
		if (me.fogTaken) { if (!world.globalFog) world.globalFog = me.fog; me.fogTaken = false; }
	}
	// A map with a fog of its own starts every frame in the colour of that fog (RB_DrawBuffer). That is what is
	// seen behind the far plane, where such a map is cut.
	if (world.globalFog && fogShown()) {
		const c = world.globalFog.color;
		gl.clearColor(c[0], c[1], c[2], 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
	}
	const s = p ? portalSplit(me, world.scripted) : null;
	if (!s) return false;

	// The eye of the room is where the portal is. With a misc_skyportal_orient it follows the viewer: by how far
	// he is from that entity times its scale. It looks the way the viewer looks, with his angle of view.
	const mainVp = viewProj, mainInv = invViewProj, mainEye = eye, defer = deferLate;
	for (let i = 0; i < 3; i++) {
		portalEye[i] = p.origin[i] + (p.orient ? (mainEye[i] - p.orient[i]) * p.scale : 0);
		portalMove[i] = portalEye[i] - mainEye[i];
	}
	eye = portalEye;
	viewProj = viewWithFar(viewMoved(mainVp, portalMove, portalVp), farClip(me.zone || world.bounds, eye, true), portalVp);
	invViewProj = M4.invert(viewProj);
	// the room with its sky: what is solid of it and what is see-through
	skyPortalOn = false;
	world.scripted = s.room;
	drawWorldScripted(now, false);
	drawWorldScripted(now, true);
	skyPortalOn = true;
	// the see-through part ends the collecting of see-through model surfaces: that is for the view of the map
	deferLate = defer;
	// between the two views the depth is cleared and nothing else (RB_BeginDrawingView)
	gl.clear(gl.DEPTH_BUFFER_BIT);
	eye = mainEye;
	viewProj = mainVp;
	invViewProj = mainInv;
	if (p.onlyFogHere && world.globalFog) { me.fog = world.globalFog; world.globalFog = null; me.fogTaken = true; }
	// What the view of the map draws: not the room, which no place of the map sees. From inside the room it is
	// the room alone, and an eye in no cluster sees everything, as in the engine (R_MarkLeaves).
	const k = me.zone ? pointCluster(me.tree, eye[0], eye[1], eye[2], false) : -1;
	world.scripted = k < 0 ? s.src : me.zone.flags[k] ? s.room : s.rest;
	return true;
}

/* the static models */

// The models of misc_model_static as SP_misc_model_static sets them up, for the list of them: where each is, its
// matrix (the axes of its angles, each scaled by its own number), the point it is culled by and its scale, and
// the model files with the entities that use each.
function staticList(list) {
	const n = list.length, v = [0, 0, 0], byPath = new Map();
	const st = { count: n, mats: [], org: new Float32Array(n * 3), cull: new Float32Array(n * 4), scale: new Float32Array(n * 3), cluster: new Int32Array(n), model: new Uint16Array(n),
		models: [], pending: true, treeOf: null, shown: 0 };
	list.forEach((e, i) => {
		const path = spawnKey(e, 'model'), key = path.toLowerCase();
		if (!byPath.has(key)) { byPath.set(key, st.models.length); st.models.push({ path, model: null, state: 0, mins: [0, 0, 0], maxs: [0, 0, 0], ents: [] }); }
		st.model[i] = byPath.get(key);
		st.models[st.model[i]].ents.push(i);
		spawnVector(e, 'origin', v);
		st.org.set(v, i * 3);
		// "zoffset" moves the point the model is culled by, not the model
		st.cull.set([v[0], v[1], v[2] + 1 + spawnFloat(e, 'zoffset')], i * 4);
		const org = v.slice();
		// "angles", or "angle" for the yaw alone
		if (!spawnVector(e, 'angles', v) && spawnKey(e, 'angle') !== undefined) v[1] = spawnFloat(e, 'angle');
		const m = anglesMatrix(org, v, 1);
		// "modelscale_vec", or "modelscale" for all three
		if (!spawnVector(e, 'modelscale_vec', v, 1) && spawnKey(e, 'modelscale') !== undefined) v[0] = v[1] = v[2] = spawnFloat(e, 'modelscale');
		for (let a = 0; a < 3; a++) for (let c = 0; c < 3; c++) m[a * 4 + c] *= v[a];
		st.scale.set(v, i * 3);
		st.mats.push(m);
	});
	return st;
}

// The box around a model from the head of its file, as R_ModelBounds gives it to the client game: the one of
// the first frame of an md3. The loader of the page does not keep it, so the head is read here, and the model
// is asked for when it is there: the file comes out of the cache of the browser then.
async function staticHead(m) {
	try {
		const dv = new DataView(await fetchGameFile(m.path));
		// what is no md3 (a Ghoul2 model) is not drawn as an entity without an instance of it (R_AddEntitySurfaces)
		if (readStr(dv, 0, 4) !== 'IDP3') { m.state = 9; return; }
		// where its frames begin: each starts with the two corners of its box
		const frames = dv.getInt32(92, true);
		for (let i = 0; i < 3; i++) { m.mins[i] = dv.getFloat32(frames + i * 4, true); m.maxs[i] = dv.getFloat32(frames + 12 + i * 4, true); }
		m.state = 2;
	} catch (e) { m.state = 9; }
}

// The models on their way, a few at a time: the loader starts on a file the moment it is asked for it. The
// states of a model: 0 not asked for, 1 its head is read, 2 the loader has it, 3 there, 9 not to be had.
const STATIC_LOADS = 4;
function pumpStatics(st) {
	let busy = 0, open = 0;
	for (const m of st.models) {
		if (m.state === 2) {
			const model = staticNow(m.path);
			if (model) {
				m.model = model;
				m.state = 3;
				// RadiusFromBounds of the box times the scale of the entity
				for (const i of m.ents) {
					let r = 0;
					for (let a = 0; a < 3; a++) r += Math.max(Math.abs(m.mins[a] * st.scale[i * 3 + a]), Math.abs(m.maxs[a] * st.scale[i * 3 + a])) ** 2;
					st.cull[i * 4 + 3] = Math.sqrt(r);
				}
			} else if (staticReady.has(m.path)) m.state = 9;
		}
		if (m.state === 0 && busy < STATIC_LOADS) { m.state = 1; staticHead(m); }
		if (m.state === 1 || m.state === 2) busy++;
		if (m.state < 3) open++;
	}
	st.pending = open > 0;
}

// the four sides of a view as planes through its eye, pointing inwards (CG_SetupFrustum), from its matrix
const viewSides = new Float32Array(16);
function setViewSides(m) {
	for (let i = 0; i < 4; i++) {
		const r = i >> 1, s = i & 1 ? -1 : 1;
		const x = m[3] + s * m[r], y = m[7] + s * m[4 + r], z = m[11] + s * m[8 + r], l = Math.hypot(x, y, z);
		viewSides[i * 4] = x / l; viewSides[i * 4 + 1] = y / l; viewSides[i * 4 + 2] = z / l; viewSides[i * 4 + 3] = (m[15] + s * m[12 + r]) / l;
	}
}

// The models of misc_model_static (CG_DrawMiscStaticModels in cg_draw.c): those whose ball is in the view
// (CG_CullPointAndRadius), whose point is seen from where the eye is (R_inPVS) and which are no further away
// than the distanceCull of the map. Around the map from above the last two do not count (see camInMap), but
// for a model whose point is in no cluster: the game shows it from next to nowhere, and the page does not.
function drawWorldEnts(now) {
	const me = state ? mapEntsFor(state.gametype) : null, st = me && me.st;
	if (!st || !st.count) return;
	if (st.pending) pumpStatics(st);
	const tree = mapTree(me), inMap = camInMap(), P = viewSides, cull = st.cull, org = st.org, far = me.distanceCull;
	if (tree && st.treeOf !== tree) {
		st.treeOf = tree;
		for (let i = 0; i < st.count; i++) st.cluster[i] = pointCluster(tree, cull[i * 4], cull[i * 4 + 1], cull[i * 4 + 2], true);
	}
	// R_inPVS: the row of the cluster the eye is in, the first row for an eye in none (CM_ClusterPVS). A point in
	// no cluster reads the bit before that row: the last one of the row above, which a map only has when the
	// number of its clusters fills its rows, and nothing for the first row. A map without visibility shows all.
	const vised = !!tree && tree.clusters > 0;
	let row = 0, none = 0;
	if (vised && inMap) {
		row = Math.max(0, pointCluster(tree, eye[0], eye[1], eye[2], true)) * tree.rowBytes;
		none = row ? tree.vis[row - 1] & 128 : 0;
	}
	setViewSides(viewProj);
	st.shown = 0;
	for (let i = 0; i < st.count; i++) {
		const m = st.models[st.model[i]];
		if (m.state !== 3) continue;
		const x = cull[i * 4], y = cull[i * 4 + 1], z = cull[i * 4 + 2], r = cull[i * 4 + 3], k = st.cluster[i];
		if (r && (P[0] * x + P[1] * y + P[2] * z + P[3] < -r || P[4] * x + P[5] * y + P[6] * z + P[7] < -r || P[8] * x + P[9] * y + P[10] * z + P[11] < -r || P[12] * x + P[13] * y + P[14] * z + P[15] < -r)) continue;
		if (vised && (k < 0 ? !none : inMap && !(tree.vis[row + (k >> 3)] & (1 << (k & 7))))) continue;
		if (inMap && Math.hypot(org[i * 3] - eye[0], org[i * 3 + 1] - eye[1], org[i * 3 + 2] - eye[2]) - r > far) continue;
		drawStatic(m.model, st.mats[i]);
		st.shown++;
	}
}
