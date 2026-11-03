// From VisionJK's web admin page (codemp/game/webadmin/page/js/bsp.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: changed: each batch keeps where each surface's triangles are (ranges), so a map edit can hide one.
/* ------------------------------------------------------------------ */
/* BSP loading                                                         */
/* ------------------------------------------------------------------ */

const SURF_SKY = 0x2000, SURF_NODRAW = 0x200000;
const MST_PLANAR = 1, MST_PATCH = 2, MST_TRIANGLE_SOUP = 3, MST_FLARE = 4;
const LIGHTMAP_BY_VERTEX = -3, LIGHTMAP_2D = -4, LS_UNUSED = 0xfe;
const VERT_SIZE = 80, SURF_SIZE = 148, SHADER_SIZE = 72;

function parseBsp(buffer) {
	const dv = new DataView(buffer);
	const ident = String.fromCharCode(dv.getUint8(0), dv.getUint8(1), dv.getUint8(2), dv.getUint8(3));
	if (ident !== 'RBSP') throw new Error('not a Jedi Academy map (' + ident + ')');
	const lump = n => ({ ofs: dv.getInt32(8 + n * 8, true), len: dv.getInt32(12 + n * 8, true) });
	// The entities of the map file as the text they are: the client game reads its own entities from it (the sky
	// portal, the models of misc_model_static) and the keys of the worldspawn, and so does the page.
	const entLump = lump(0), entityText = new TextDecoder('latin1').decode(new Uint8Array(buffer, entLump.ofs, Math.max(0, entLump.len))).replace(/\0+$/, '');

	const sl = lump(1), shaders = [];
	for (let o = sl.ofs; o < sl.ofs + sl.len; o += SHADER_SIZE) {
		let name = '';
		for (let i = 0; i < 64; i++) { const ch = dv.getUint8(o + i); if (!ch) break; name += String.fromCharCode(ch); }
		shaders.push({ name, flags: dv.getInt32(o + 64, true), contents: dv.getInt32(o + 68, true) });
	}

	const vl = lump(10), numVerts = Math.floor(vl.len / VERT_SIZE);
	const il = lump(11), indexes = new Int32Array(buffer.slice(il.ofs, il.ofs + (il.len & ~3)));
	const ml = lump(7);
	const bounds = { mins: [0, 1, 2].map(i => dv.getFloat32(ml.ofs + i * 4, true)), maxs: [0, 1, 2].map(i => dv.getFloat32(ml.ofs + 12 + i * 4, true)) };
	// every brush model: the world first, then the ones an entity names with *1, *2, ...
	const models = [];
	for (let o = ml.ofs; o + 40 <= ml.ofs + ml.len; o += 40) {
		models.push({ mins: [0, 1, 2].map(i => dv.getFloat32(o + i * 4, true)), maxs: [0, 1, 2].map(i => dv.getFloat32(o + 12 + i * 4, true)),
			firstSurface: dv.getInt32(o + 24, true), numSurfaces: dv.getInt32(o + 28, true),
			firstBrush: dv.getInt32(o + 32, true), numBrushes: dv.getInt32(o + 36, true) });
	}
	// The brushes: what the game collides with, seen or not. Kept as they are in the file (a plane is a normal
	// and a distance, a brush its first side, how many it has and its shader, a side its plane and two more
	// numbers). Their edges are worked out when a view of the editor asks for them.
	const pl = lump(2), bl = lump(8), bsl = lump(9);
	const brushes = {
		planes: new Float32Array(buffer.slice(pl.ofs, pl.ofs + (pl.len & ~3))),
		brushes: new Int32Array(buffer.slice(bl.ofs, bl.ofs + (bl.len & ~3))),
		sides: new Int32Array(buffer.slice(bsl.ofs, bsl.ofs + (bsl.len & ~3)))
	};

	// The vertices as the file has them, 80 bytes each: 20 floats (place 0, st 3, the st of four lightmaps from 5,
	// normal 13) of which the last four are the bytes of four colours. A lump that does not begin at a whole
	// float is copied to one that does.
	const vbuf = vl.ofs & 3 ? buffer.slice(vl.ofs, vl.ofs + numVerts * VERT_SIZE) : buffer, vofs = vl.ofs & 3 ? 0 : vl.ofs;
	const vf = new Float32Array(vbuf, vofs, numVerts * 20), vb = new Uint8Array(vbuf, vofs, numVerts * VERT_SIZE);

	const ll = lump(14), lmSize = 128 * 128 * 3, lightmaps = [];
	for (let o = ll.ofs; o + lmSize <= ll.ofs + ll.len; o += lmSize) lightmaps.push(new Uint8Array(buffer, o, lmSize));

	// The surfaces, as R_LoadSurfaces in tr_bsp.cpp takes them. What decides whether one is drawn is what decides
	// it in the engine: a patch of a shader the map file marks nodraw is left out (ParseMesh), every face and
	// every model of the map is kept and drawn with whatever stages its shader has, also none. A surface the
	// file marks as sky is kept apart (sky: true on its batch): whether it is the sky, which is drawn around the
	// eye, or a surface like any other is for the script of its shader to say (settleSky).
	const sfl = lump(13), numSurfs = Math.floor(sfl.len / SURF_SIZE);
	const surfModel = new Uint16Array(numSurfs);
	models.forEach((m, n) => { if (n) surfModel.fill(n, Math.max(0, m.firstSurface), Math.min(numSurfs, m.firstSurface + m.numSurfaces)); });
	let sky = '';
	const drawn = [], flares = [], grids = new Array(numSurfs).fill(null), gridList = [], groups = new Map();
	const points = new Float32Array(32 * 32 * GV);	// MAX_PATCH_SIZE each way
	for (let n = 0; n < numSurfs; n++) {
		const o = sfl.ofs + n * SURF_SIZE;
		const shaderNum = dv.getInt32(o, true), fogNum = dv.getInt32(o + 4, true), type = dv.getInt32(o + 8, true);
		const firstVert = dv.getInt32(o + 12, true), nv = dv.getInt32(o + 16, true);
		const firstIndex = dv.getInt32(o + 20, true), ni = dv.getInt32(o + 24, true);
		const lmNum = dv.getInt32(o + 36, true);
		const shader = shaders[shaderNum];
		if (!shader) continue;
		if ((shader.flags & SURF_SKY) && !sky) sky = shader.name;
		if (type === MST_FLARE) {
			// ParseFlare: where it is, its colour (which the engine never uses) and the way it faces
			const f3 = at => [dv.getFloat32(o + at, true), dv.getFloat32(o + at + 4, true), dv.getFloat32(o + at + 8, true)];
			flares.push({ surface: n, shader: shaderNum, fog: fogNum, model: surfModel[n], origin: f3(92), color: f3(104), normal: f3(128) });
			continue;
		}
		if (type !== MST_PLANAR && type !== MST_TRIANGLE_SOUP && type !== MST_PATCH) continue;
		// A model of the map is always lit by its vertices (ParseTriSurf), whatever lightmap the file names
		const lm = type !== MST_TRIANGLE_SOUP && lmNum >= 0 && lmNum < lightmaps.length ? lmNum : -1;
		// The colours of a surface that is lit by its vertices go through the light styles (ComputeFinalVertexColor
		// in tr_surface.cpp): times the colour of the style, shifted right by eight. The style of the light that
		// is always on is white, 255, which makes every colour one less. No style at all is black.
		// (a lightmap the file does not have, or a number that is none, is taken as lit by the vertices: R_FindLightmap)
		const byVertex = type === MST_TRIANGLE_SOUP || lmNum === LIGHTMAP_BY_VERTEX || lmNum >= lightmaps.length || lmNum < LIGHTMAP_2D;
		const s = { n, type, shaderNum, fog: fogNum, lm, lmNum, model: surfModel[n], firstVert, nv, firstIndex, ni, grid: false, style: !byVertex ? 256 : dv.getUint8(o + 32) < LS_UNUSED ? 255 : 0,
			pw: dv.getInt32(o + 140, true), ph: dv.getInt32(o + 144, true) };
		if (type === MST_PATCH) {
			if (shader.flags & SURF_NODRAW) continue;
			if (s.pw < 3 || s.ph < 3 || s.pw > 32 || s.ph > 32 || firstVert < 0 || firstVert + s.pw * s.ph > numVerts) continue;
			for (let i = 0, e = s.pw * s.ph; i < e; i++) {
				const v = (firstVert + i) * 20, c = (firstVert + i) * VERT_SIZE + 64, p = i * GV;
				points[p] = vf[v]; points[p + 1] = vf[v + 1]; points[p + 2] = vf[v + 2];
				points[p + 3] = vf[v + 3]; points[p + 4] = vf[v + 4]; points[p + 5] = vf[v + 5]; points[p + 6] = vf[v + 6];
				points[p + 7] = vf[v + 13]; points[p + 8] = vf[v + 14]; points[p + 9] = vf[v + 15];
				points[p + 10] = vb[c]; points[p + 11] = vb[c + 1]; points[p + 12] = vb[c + 2]; points[p + 13] = vb[c + 3];
			}
			const g = subdividePatch(s.pw, s.ph, points);
			// ParseMesh: the middle and the radius of all the patches that must lose their detail together, which
			// the map file keeps in two vectors of the surface that a patch has no other use for
			const lo = gridLod, f = Math.fround;
			for (let i = 0; i < 3; i++) { lo[i] = dv.getFloat32(o + 104 + i * 4, true); lo[3 + i] = lo[i] + dv.getFloat32(o + 116 + i * 4, true); lo[3 + i] = lo[3 + i] * 0.5; lo[6 + i] = lo[i] - lo[3 + i]; }
			g.lodOrigin = [lo[3], lo[4], lo[5]];
			g.lodRadius = f(Math.sqrt(f(f(f(lo[6] * lo[6]) + f(lo[7] * lo[7])) + f(lo[8] * lo[8]))));
			g.group = g.lodOrigin.join(' ') + ' ' + g.lodRadius;
			grids[n] = g;
			gridList.push(n);
			if (!groups.has(g.group)) groups.set(g.group, []);
			groups.get(g.group).push(n);
			s.grid = true;
		} else if (firstVert < 0 || nv < 0 || firstVert + nv > numVerts || firstIndex < 0 || ni < 3 || firstIndex + ni > indexes.length) {
			continue;
		}
		s.sky = !!(shader.flags & SURF_SKY);
		drawn.push(s);
	}
	stitchAllPatches(grids, gridList, groups);
	fixSharedLodError(grids, gridList, groups);

	// batches per shader + lightmap, and per brush model: one that is an entity (a door, a lift) is drawn where
	// that entity is, the world (0) where the file has it
	const batchOf = new Map(), batches = [];
	let allVerts = 0, allIdx = 0;
	for (const s of drawn) {
		const g = s.grid ? grids[s.n] : null;
		s.verts = g ? g.width * g.height : s.nv;
		s.idx = g ? (g.width - 1) * (g.height - 1) * 6 : s.ni - s.ni % 3;
		const key = s.shaderNum + ':' + s.lm + ':' + s.fog + ':' + s.model;
		let b = batchOf.get(key);
		if (!b) {
			b = { shader: s.shaderNum, fog: s.fog, model: s.model, lightmap: s.lm, sky: s.sky, verts: 0, count: 0, surfs: [], grids: [], ranges: [] };
			batchOf.set(key, b);
			batches.push(b);
		}
		b.surfs.push(s);
		b.verts += s.verts; b.count += s.idx;
		if (!s.sky) { allVerts += s.verts; allIdx += s.idx; }
	}
	// what a surface is made of, for the click that asks: its shader, lightmap, kind, fog, brush model and size.
	// The solid surfaces for the ray of a click are all of them but the ones marked as sky, patches with every
	// point they have.
	const surfs = [], pickPos = new Float32Array(allVerts * 3), pickIdx = new Uint32Array(allIdx), pickModel = new Uint16Array(allIdx / 3), pickSurf = new Uint32Array(allIdx / 3);
	for (const s of drawn) if (!s.sky) { s.at = surfs.length; surfs.push(null); }
	let pv = 0, pi = 0;
	for (const b of batches) {
		const pos = b.pos = new Float32Array(b.verts * 3), st = b.st = new Float32Array(b.verts * 2), lm = b.lm = new Float32Array(b.verts * 2);
		const col = b.col = new Uint8Array(b.verts * 4), norm = b.norm = new Float32Array(b.verts * 3), idx = b.idx = new Uint32Array(b.count);
		let nv = 0, ni = 0;
		for (const s of b.surfs) {
			const g = s.grid ? grids[s.n] : null, base = nv, first = ni, style = s.style;
			if (g) {
				const gv = g.verts, w = g.width, h = g.height;
				for (let i = 0, p = 0; i < s.verts; i++, p += GV, nv++) {
					pos[nv * 3] = gv[p]; pos[nv * 3 + 1] = gv[p + 1]; pos[nv * 3 + 2] = gv[p + 2];
					st[nv * 2] = gv[p + 3]; st[nv * 2 + 1] = gv[p + 4]; lm[nv * 2] = gv[p + 5]; lm[nv * 2 + 1] = gv[p + 6];
					norm[nv * 3] = gv[p + 7]; norm[nv * 3 + 1] = gv[p + 8]; norm[nv * 3 + 2] = gv[p + 9];
					for (let c = 0; c < 3; c++) col[nv * 4 + c] = style > 255 ? gv[p + 10 + c] : gv[p + 10 + c] * style >> 8;
					col[nv * 4 + 3] = gv[p + 13];
				}
				// every row and column, in the order of RB_SurfaceGrid. Which of them a frame draws is set there
				// (updateGridLod), within these same places of the list.
				ni = gridIndexes(idx, ni, base, w, null, w, null, h);
				// low and high: the errors between which the rows and columns that are in the list now are the right
				// ones, cols and rows: how many those are
				let low = 0;
				for (let i = 1; i < w - 1; i++) low = Math.max(low, g.widthLodError[i]);
				for (let i = 1; i < h - 1; i++) low = Math.max(low, g.heightLodError[i]);
				b.grids.push({ first, base, width: w, height: h, widthLodError: g.widthLodError, heightLodError: g.heightLodError, lodOrigin: g.lodOrigin, lodRadius: g.lodRadius, model: s.model, surface: s.n, low, high: Infinity, cols: w, rows: h });
			} else {
				for (let i = 0; i < s.verts; i++, nv++) {
					const v = (s.firstVert + i) * 20, c = (s.firstVert + i) * VERT_SIZE + 64;
					pos[nv * 3] = vf[v]; pos[nv * 3 + 1] = vf[v + 1]; pos[nv * 3 + 2] = vf[v + 2];
					st[nv * 2] = vf[v + 3]; st[nv * 2 + 1] = vf[v + 4]; lm[nv * 2] = vf[v + 5]; lm[nv * 2 + 1] = vf[v + 6];
					norm[nv * 3] = vf[v + 13]; norm[nv * 3 + 1] = vf[v + 14]; norm[nv * 3 + 2] = vf[v + 15];
					for (let k = 0; k < 3; k++) col[nv * 4 + k] = style > 255 ? vb[c + k] : vb[c + k] * style >> 8;
					col[nv * 4 + 3] = vb[c + 3];
				}
				for (let i = 0; i < s.idx; i++) idx[ni++] = base + indexes[s.firstIndex + i];
			}
			// SJK: where each surface's triangles are in the batch's list, so a map edit can hide one
			b.ranges.push([s.n, first, ni - first, base]);
			if (s.sky) continue;
			pickPos.set(pos.subarray(base * 3, nv * 3), pv * 3);
			for (let i = first; i < ni; i++) pickIdx[pi + i - first] = idx[i] - base + pv;
			pickModel.fill(s.model, pi / 3, (pi + s.idx) / 3);
			pickSurf.fill(s.at, pi / 3, (pi + s.idx) / 3);
			pv += s.verts; pi += s.idx;
			// grid: the columns and rows the engine makes of a patch, surface: its number in the map file
			surfs[s.at] = { shader: s.shaderNum, lm: s.lmNum, type: s.type, fog: s.fog, model: s.model, verts: s.verts, tris: s.idx / 3, patch: s.type === MST_PATCH ? [s.pw, s.ph] : null, grid: g ? [g.width, g.height] : null, surface: s.n };
		}
		b.surfs = null;
	}
	// The click wants the triangles of one brush model after the other, which the batches are not sorted by
	const pickRuns = [];
	for (let i = 0; i < pickModel.length; i++) {
		const run = pickRuns[pickRuns.length - 1];
		if (run && run[0] === pickModel[i]) run[2] = (i + 1) * 3; else pickRuns.push([pickModel[i], i * 3, (i + 1) * 3]);
	}

	// The fogs (R_LoadFogs in tr_bsp.cpp): a shader with fogParms and the brush that is filled with it. No brush
	// means the whole map. The side of the brush that is seen from outside is the plane the fog ends at, kept as
	// the engine keeps it: looking into the fog.
	const fl = lump(12), fogs = [];
	for (let o = fl.ofs; o + 72 <= fl.ofs + fl.len; o += 72) {
		let name = '';
		for (let i = 0; i < 64; i++) { const ch = dv.getUint8(o + i); if (!ch) break; name += String.fromCharCode(ch); }
		const brush = dv.getInt32(o + 64, true), side = dv.getInt32(o + 68, true);
		const pn = brush >= 0 && side >= 0 ? brushes.sides[(brushes.brushes[brush * 3] + side) * 3] * 4 : -1, P = brushes.planes;
		fogs.push({ shader: name, brush, side, global: brush === -1, plane: pn >= 0 && pn + 3 < P.length ? [-P[pn], -P[pn + 1], -P[pn + 2], -P[pn + 3]] : [0, 0, 0, 0] });
	}
	// The light grid: the light a model gets where it stands (R_LoadLightGrid in tr_bsp.cpp). The map is cut into
	// cells, 64 x 64 x 128 units unless the worldspawn says otherwise, every cell names its sample in the array,
	// and a sample is the ambient and the directed light of four styles, the styles, and where the light comes from.
	const el = lump(0), gdl = lump(15), gal = lump(17);
	const worldspawn = /\{([^}]*)\}/.exec(new TextDecoder('latin1').decode(new Uint8Array(buffer, el.ofs, Math.min(el.len, 8192))));
	const gridKey = worldspawn && /"gridsize"\s+"([^"]*)"/i.exec(worldspawn[1]);
	const cell = gridKey ? gridKey[1].trim().split(/\s+/).map(Number) : [];
	const gridSize = cell.length === 3 && cell.every(v => v > 0) ? cell : [64, 64, 128];
	const gridOrigin = gridSize.map((g, i) => g * Math.ceil(models[0].mins[i] / g));
	const grid = {
		size: gridSize, origin: gridOrigin,
		bounds: gridSize.map((g, i) => Math.floor((g * Math.floor(models[0].maxs[i] / g) - gridOrigin[i]) / g) + 1),
		data: new Uint8Array(buffer.slice(gdl.ofs, gdl.ofs + gdl.len)),
		array: new Uint16Array(buffer.slice(gal.ofs, gal.ofs + (gal.len & ~1)))
	};
	const stats = { surfaces: numSurfs, vertices: numVerts, indexes: indexes.length, shaders: shaders.length, lightmaps: lightmaps.length,
		brushModels: models.length, brushes: brushes.brushes.length / 3, planes: brushes.planes.length / 4, bytes: buffer.byteLength, patches: gridList.length, flares: flares.length };
	return { entityText, shaders, lightmaps, batches, bounds, sky, models, brushes, fogs, grid, stats, surfs, flares, pick: { pos: pickPos, idx: pickIdx, model: pickModel, surf: pickSurf, runs: pickRuns } };
}

// ---- curved surfaces as the engine makes them: R_SubdividePatchToGrid and what it calls in rd-vanilla/tr_curve.cpp,
// the stitching of neighbours in tr_bsp.cpp. Every sum here is rounded to a float where the engine has one, so
// that a patch which is just at a limit falls to the same side of it.

const GRID_MAX = 65;			// MAX_GRID_SIZE: the most points a grid has along a side
const GV = 14;					// a point of a grid as floats: place, st 3, lightmap st 5, normal 7, colour 10
const GRID_ROW = GRID_MAX * GV;
const SUBDIVISIONS = 4;			// r_subdivisions: how far a curve may be off its lines before it gets more of them
// the work space of the engine: the points row by row, GRID_MAX to a row, and the error tables of both ways
const gridCtrl = new Float32Array(GRID_MAX * GRID_ROW), gridErr = [new Float32Array(GRID_MAX), new Float32Array(GRID_MAX)];
const gridTemp = new Float32Array(GV * 3), gridVec = new Float32Array(9), gridLod = new Float32Array(9);
const gridAround = new Float32Array(24), gridGood = new Uint8Array(8);
const GRID_NEIGHBORS = [0, 1, 1, 1, 1, 0, 1, -1, 0, -1, -1, -1, -1, 0, -1, 1];
// What the engine reads one float past the end of an error table (R_StitchPatches does, going backwards): the
// mark the zone memory of the engine ends every block with, 0x21436587, which as a float is next to nothing.
const GRID_PAST_END = new Float32Array(new Uint32Array([0x21436587]).buffer)[0];

// LerpDrawVert: the point halfway between two. The colours are whole numbers and are halved as such.
function gridLerp(a, ao, b, bo, out, oo) {
	for (let k = 0; k < 10; k++) out[oo + k] = 0.5 * (a[ao + k] + b[bo + k]);
	for (let k = 10; k < GV; k++) out[oo + k] = (a[ao + k] + b[bo + k]) >> 1;
}

// Transpose: columns become rows
function gridTranspose(width, height) {
	const c = gridCtrl, t = gridTemp;
	const wide = width > height;
	for (let i = 0, n = wide ? height : width, e = wide ? width : height; i < n; i++) {
		for (let j = i + 1; j < e; j++) {
			// wide: ctrl[j][i] from ctrl[i][j], else ctrl[i][j] from ctrl[j][i]
			const to = wide ? j * GRID_ROW + i * GV : i * GRID_ROW + j * GV, from = wide ? i * GRID_ROW + j * GV : j * GRID_ROW + i * GV;
			if (j < n) {
				for (let k = 0; k < GV; k++) { t[k] = c[to + k]; c[to + k] = c[from + k]; c[from + k] = t[k]; }
			} else {
				c.copyWithin(to, from, from + GV);
			}
		}
	}
}

// PutPointsOnCurve: the points between the ones the curve goes through are put on it
function gridOnCurve(width, height) {
	const c = gridCtrl, t = gridTemp;
	for (let i = 0; i < width; i++) {
		for (let j = 1; j < height; j += 2) {
			const o = j * GRID_ROW + i * GV;
			gridLerp(c, o, c, o + GRID_ROW, t, 0);
			gridLerp(c, o, c, o - GRID_ROW, t, GV);
			gridLerp(t, 0, t, GV, c, o);
		}
	}
	for (let j = 0; j < height; j++) {
		for (let i = 1; i < width; i += 2) {
			const o = j * GRID_ROW + i * GV;
			gridLerp(c, o, c, o + GV, t, 0);
			gridLerp(c, o, c, o - GV, t, GV);
			gridLerp(t, 0, t, GV, c, o);
		}
	}
}

// MakeMeshNormals: the normal of a point from the points around it, around the seam where a patch is closed
function gridNormals(width, height) {
	const c = gridCtrl, f = Math.fround, around = gridAround, good = gridGood;
	const apart = (a, b) => { const x = f(c[a] - c[b]), y = f(c[a + 1] - c[b + 1]), z = f(c[a + 2] - c[b + 2]); return f(f(f(x * x) + f(y * y)) + f(z * z)) > 1; };
	let wrapWidth = true, wrapHeight = true;
	for (let i = 0; i < height && wrapWidth; i++) if (apart(i * GRID_ROW, i * GRID_ROW + (width - 1) * GV)) wrapWidth = false;
	for (let i = 0; i < width && wrapHeight; i++) if (apart(i * GV, (height - 1) * GRID_ROW + i * GV)) wrapHeight = false;
	for (let i = 0; i < width; i++) {
		for (let j = 0; j < height; j++) {
			const o = j * GRID_ROW + i * GV;
			for (let k = 0; k < 8; k++) {
				good[k] = 0;
				for (let dist = 1; dist <= 3; dist++) {
					let x = i + GRID_NEIGHBORS[k * 2] * dist, y = j + GRID_NEIGHBORS[k * 2 + 1] * dist;
					if (wrapWidth) { if (x < 0) x = width - 1 + x; else if (x >= width) x = 1 + x - width; }
					if (wrapHeight) { if (y < 0) y = height - 1 + y; else if (y >= height) y = 1 + y - height; }
					if (x < 0 || x >= width || y < 0 || y >= height) break;		// the edge of the patch
					const q = y * GRID_ROW + x * GV, tx = f(c[q] - c[o]), ty = f(c[q + 1] - c[o + 1]), tz = f(c[q + 2] - c[o + 2]);
					const len = f(Math.sqrt(f(f(f(tx * tx) + f(ty * ty)) + f(tz * tz))));
					if (len === 0) continue;		// the same point: one further
					const il = f(1 / len);
					around[k * 3] = tx * il; around[k * 3 + 1] = ty * il; around[k * 3 + 2] = tz * il;
					good[k] = 1;
					break;
				}
			}
			let sx = 0, sy = 0, sz = 0;
			for (let k = 0; k < 8; k++) {
				const a = ((k + 1) & 7) * 3, b = k * 3;
				if (!good[k] || !good[(k + 1) & 7]) continue;
				const nx = f(f(around[a + 1] * around[b + 2]) - f(around[a + 2] * around[b + 1]));
				const ny = f(f(around[a + 2] * around[b]) - f(around[a] * around[b + 2]));
				const nz = f(f(around[a] * around[b + 1]) - f(around[a + 1] * around[b]));
				const len = f(Math.sqrt(f(f(f(nx * nx) + f(ny * ny)) + f(nz * nz))));
				if (len === 0) continue;
				const il = f(1 / len);
				sx = f(sx + f(nx * il)); sy = f(sy + f(ny * il)); sz = f(sz + f(nz * il));
			}
			const len = f(Math.sqrt(f(f(f(sx * sx) + f(sy * sy)) + f(sz * sz)))), il = len ? f(1 / len) : 0;
			c[o + 7] = sx * il; c[o + 8] = sy * il; c[o + 9] = sz * il;
		}
	}
}

// R_CreateSurfaceGridMesh: the grid that is in the work space, as an object of its own
function gridMake(width, height) {
	const verts = new Float32Array(width * height * GV);
	for (let j = 0; j < height; j++) verts.set(gridCtrl.subarray(j * GRID_ROW, j * GRID_ROW + width * GV), j * width * GV);
	// merged: whether two points of an edge are the same place, asked for often and worked out once (-1: not yet)
	return { width, height, verts, widthLodError: gridErr[0].slice(0, width), heightLodError: gridErr[1].slice(0, height), lodOrigin: null, lodRadius: 0, group: '', lodStitched: false, lodFixed: 0, merged: [-1, -1, -1, -1] };
}

// R_SubdividePatchToGrid: the control points of a patch, width x height of them row by row, into the grid the
// engine draws. A row or column is put in wherever the curve is more than r_subdivisions off the line between
// its points, rows and columns that are straight are taken out, and every point remembers how far off the curve
// was that made it (as one over that), which is what the level of detail goes by.
function subdividePatch(width, height, points) {
	const c = gridCtrl, t = gridTemp, v = gridVec, f = Math.fround, tooSmall = f(0.1);
	for (let j = 0; j < height; j++) c.set(points.subarray(j * width * GV, (j + 1) * width * GV), j * GRID_ROW);
	for (let dir = 0; dir < 2; dir++) {
		const err = gridErr[dir];
		err.fill(0);
		for (let j = 0; j + 2 < width; j += 2) {
			// how far the middle of the curve is off the line from its first to its last point, the most of all rows
			let maxLen = 0;
			for (let i = 0; i < height; i++) {
				const a = i * GRID_ROW + j * GV, b = a + GV, e = b + GV;
				for (let l = 0; l < 3; l++) {
					v[l] = f(f(c[a + l] + c[b + l] * 2) + c[e + l]) * 0.25;
					v[l] = v[l] - c[a + l];
					v[3 + l] = c[e + l] - c[a + l];
				}
				const length = f(Math.sqrt(f(f(f(v[3] * v[3]) + f(v[4] * v[4])) + f(v[5] * v[5]))));
				if (length) { const il = f(1 / length); v[3] = v[3] * il; v[4] = v[4] * il; v[5] = v[5] * il; }
				const d = f(f(f(v[0] * v[3]) + f(v[1] * v[4])) + f(v[2] * v[5]));
				for (let l = 0; l < 3; l++) { v[6 + l] = v[3 + l] * d; v[l] = v[l] - v[6 + l]; }
				const len = f(f(f(v[0] * v[0]) + f(v[1] * v[1])) + f(v[2] * v[2]));
				if (len > maxLen) maxLen = len;
			}
			maxLen = f(Math.sqrt(maxLen));
			// all of them on the line: the column goes
			if (maxLen < tooSmall) { err[j + 1] = 999; continue; }
			// no room for more, or near enough
			if (width + 2 > GRID_MAX || maxLen <= SUBDIVISIONS) { err[j + 1] = 1 / maxLen; continue; }
			err[j + 2] = 1 / maxLen;
			// two columns more, and the point in the middle is now on the curve
			width += 2;
			for (let i = 0; i < height; i++) {
				const a = i * GRID_ROW + j * GV;
				gridLerp(c, a, c, a + GV, t, 0);
				gridLerp(c, a + GV, c, a + GV * 2, t, GV);
				gridLerp(t, 0, t, GV, t, GV * 2);
				c.copyWithin(a + GV * 4, a + GV * 2, i * GRID_ROW + (width - 2) * GV);
				c.set(t.subarray(0, GV), a + GV);
				c.set(t.subarray(GV * 2), a + GV * 2);
				c.set(t.subarray(GV, GV * 2), a + GV * 3);
			}
			// the same place once more: it may need more
			j -= 2;
		}
		gridTranspose(width, height);
		const w = width;
		width = height;
		height = w;
	}
	gridOnCurve(width, height);
	// The columns and rows that are straight are taken out. Like the engine, the one that moves into the place
	// of one taken out is not looked at.
	for (let i = 1; i < width - 1; i++) {
		if (gridErr[0][i] !== 999) continue;
		for (let k = 0; k < height; k++) c.copyWithin(k * GRID_ROW + i * GV, k * GRID_ROW + (i + 1) * GV, k * GRID_ROW + width * GV);
		gridErr[0].copyWithin(i, i + 1, width);
		width--;
	}
	for (let i = 1; i < height - 1; i++) {
		if (gridErr[1][i] !== 999) continue;
		c.copyWithin(i * GRID_ROW, (i + 1) * GRID_ROW, height * GRID_ROW);
		gridErr[1].copyWithin(i, i + 1, height);
		height--;
	}
	// the long side becomes the rows (the engine does it for longer strips of triangles)
	if (height > width) {
		gridTranspose(width, height);
		// InvertErrorTable
		const e0 = gridErr[0].slice(), e1 = gridErr[1].slice();
		for (let i = 0; i < width; i++) gridErr[1][i] = e0[i];
		for (let i = 0; i < height; i++) gridErr[0][i] = e1[height - 1 - i];
		const w = width;
		width = height;
		height = w;
		// InvertCtrl: every row the other way round
		for (let i = 0; i < height; i++) {
			for (let j = 0; j < width >> 1; j++) {
				const a = i * GRID_ROW + j * GV, b = i * GRID_ROW + (width - 1 - j) * GV;
				for (let k = 0; k < GV; k++) { t[k] = c[a + k]; c[a + k] = c[b + k]; c[b + k] = t[k]; }
			}
		}
	}
	gridNormals(width, height);
	return gridMake(width, height);
}

// R_GridInsertColumn and R_GridInsertRow: a grid with one column (or row) more before the one numbered at, its
// points halfway between their neighbours but for the one in the row (column) numbered where, which is the
// point pv[po..] of the neighbouring grid. The normals are made anew.
function gridInsert(grid, column, at, where, pv, po, loderror) {
	const w = grid.width, h = grid.height, v = grid.verts, c = gridCtrl;
	const width = column ? w + 1 : w, height = column ? h : h + 1;
	const px = pv[po], py = pv[po + 1], pz = pv[po + 2];
	for (let j = 0; j < height; j++) {
		for (let i = 0; i < width; i++) {
			const o = j * GRID_ROW + i * GV, n = column ? i : j, oi = column && i > at ? i - 1 : i, oj = !column && j > at ? j - 1 : j;
			if (n !== at) { for (let k = 0, s = (oj * w + oi) * GV; k < GV; k++) c[o + k] = v[s + k]; continue; }
			const s = (j * w + i) * GV;
			gridLerp(v, s - (column ? GV : w * GV), v, s, c, o);
			if ((column ? j : i) === where) { c[o] = px; c[o + 1] = py; c[o + 2] = pz; }
		}
	}
	const along = column ? 0 : 1, old = column ? grid.widthLodError : grid.heightLodError;
	for (let i = 0, n = column ? width : height; i < n; i++) gridErr[along][i] = i === at ? loderror : old[i > at ? i - 1 : i];
	gridErr[1 - along].set(column ? grid.heightLodError : grid.widthLodError);
	gridNormals(width, height);
	const g = gridMake(width, height);
	g.lodOrigin = grid.lodOrigin; g.lodRadius = grid.lodRadius; g.group = grid.group;
	return g;
}

// whether two places are further apart than limit along one of the axes (orAt: or just that far)
function gridApart(a, ao, b, bo, limit, orAt = false) {
	const f = Math.fround, x = Math.abs(f(a[ao] - b[bo])), y = Math.abs(f(a[ao + 1] - b[bo + 1])), z = Math.abs(f(a[ao + 2] - b[bo + 2]));
	return orAt ? x >= limit || y >= limit || z >= limit : x > limit || y > limit || z > limit;
}

// The edges of a grid by their number: 0 its first row, 1 its last, 2 its first column, 3 its last. Where the
// edge begins in the points, how far it is from one point to the next, and how many it has.
const edgeStart = (g, e) => e === 1 ? (g.height - 1) * g.width : e === 3 ? g.width - 1 : 0;
const edgeStep = (g, e) => e < 2 ? 1 : g.width;
const edgeCount = (g, e) => e < 2 ? g.width : g.height;

// R_MergedWidthPoints and R_MergedHeightPoints: two points of an edge, its ends aside, are in the same place
function gridMerged(g, e) {
	if (g.merged[e] >= 0) return g.merged[e];
	const start = edgeStart(g, e), step = edgeStep(g, e), count = edgeCount(g, e), v = g.verts;
	let found = 0;
	for (let i = 1; i < count - 1 && !found; i++) {
		for (let j = i + 1; j < count - 1; j++) {
			if (!gridApart(v, (start + i * step) * GV, v, (start + j * step) * GV, 0.1)) { found = 1; break; }
		}
	}
	return g.merged[e] = found;
}

// R_StitchPatches: grid a has a point on an edge between two that are neighbours on an edge of grid b, so b
// would leave a crack there. b gets that point: a column or a row more. true: it got one.
// The edges of a are gone through as the engine does: its rows forwards, its columns forwards, then both backwards.
function stitchPatches(grids, na, nb) {
	const a = grids[na], b = grids[nb], va = a.verts, vb = b.verts;
	for (let pass = 0; pass < 4; pass++) {
		const back = pass > 1, errors = pass & 1 ? a.heightLodError : a.widthLodError;
		for (let n = 0; n < 2; n++) {
			const ea = (pass & 1) * 2 + n, start = edgeStart(a, ea), step = edgeStep(a, ea), count = edgeCount(a, ea);
			if (gridMerged(a, ea)) continue;
			for (let k = back ? count - 1 : 0; back ? k > 1 : k < count - 2; k += back ? -2 : 2) {
				const p0 = (start + k * step) * GV, p2 = (start + (back ? k - 2 : k + 2) * step) * GV, mid = (start + (back ? k - 1 : k + 1) * step) * GV;
				// along the rows of b a column goes in, along its columns a row
				for (let eb = 0; eb < 4; eb++) {
					if ((eb < 2 ? b.width : b.height) >= GRID_MAX) { eb |= 1; continue; }
					const sb = edgeStart(b, eb), tb = edgeStep(b, eb);
					for (let l = 0, e = edgeCount(b, eb) - 1; l < e; l++) {
						const q0 = (sb + l * tb) * GV, q1 = q0 + tb * GV;
						if (gridApart(va, p0, vb, q0, 0.1) || gridApart(va, p2, vb, q1, 0.1)) continue;
						// two points of b in the same place have nothing between them
						if (!gridApart(vb, q0, vb, q1, 0.01, true)) continue;
						// the error is read at k + 1 also on the way back, as the engine reads it
						grids[nb] = gridInsert(b, eb < 2, l + 1, eb & 1 ? (eb < 2 ? b.height : b.width) - 1 : 0, va, mid, k + 1 < count ? errors[k + 1] : GRID_PAST_END);
						return true;
					}
				}
			}
		}
	}
	return false;
}

// R_StitchAllPatches and R_TryStitchingPatch: every grid against every grid of its group (the patches that lose
// their detail together, see ParseMesh), itself among them, until nothing is put in any more. A grid that got a
// point is new and is gone through again.
function stitchAllPatches(grids, list, groups) {
	for (let stitched = true; stitched; ) {
		stitched = false;
		for (const n of list) {
			if (grids[n].lodStitched) continue;
			grids[n].lodStitched = true;
			stitched = true;
			for (const other of groups.get(grids[n].group)) while (stitchPatches(grids, n, other)) { /* until there is no crack left */ }
		}
	}
}

// R_FixSharedVertexLodError: a point two grids of a group share on their edges gets the same error in both, so
// that both leave it out at the same distance
function fixSharedLodError(grids, list, groups) {
	for (const n of list) {
		const g = grids[n];
		if (g.lodFixed) continue;
		g.lodFixed = 2;
		fixSharedLodErrorFrom(grids, groups.get(g.group), n + 1, g);
	}
}

function fixSharedLodErrorFrom(grids, members, start, a) {
	for (const nb of members) {
		const b = grids[nb];
		if (nb < start || b.lodFixed === 2) continue;
		let touch = false;
		for (let ea = 0; ea < 4; ea++) {
			if (gridMerged(a, ea)) continue;
			const sa = edgeStart(a, ea), ta = edgeStep(a, ea), errA = ea < 2 ? a.widthLodError : a.heightLodError;
			for (let k = 1, e = edgeCount(a, ea) - 1; k < e; k++) {
				for (let eb = 0; eb < 4; eb++) {
					if (gridMerged(b, eb)) continue;
					const sb = edgeStart(b, eb), tb = edgeStep(b, eb), errB = eb < 2 ? b.widthLodError : b.heightLodError;
					for (let l = 1, m = edgeCount(b, eb) - 1; l < m; l++) {
						if (gridApart(a.verts, (sa + k * ta) * GV, b.verts, (sb + l * tb) * GV, 0.1)) continue;
						errB[l] = errA[k];
						touch = true;
					}
				}
			}
		}
		if (touch) {
			b.lodFixed = 2;
			fixSharedLodErrorFrom(grids, members, start, b);
		}
	}
}

// The triangles of a grid as RB_SurfaceGrid makes them, into idx from at on: of the columns cols[0..nc) and the
// rows rows[0..nr), or of all of them where the list is null. base is the first vertex of the grid, width how
// many it has in a row. Returns where the next index goes.
function gridIndexes(idx, at, base, width, cols, nc, rows, nr) {
	for (let i = 0; i < nr - 1; i++) {
		const r0 = base + (rows ? rows[i] : i) * width, r1 = base + (rows ? rows[i + 1] : i + 1) * width;
		for (let j = 0; j < nc - 1; j++) {
			const c0 = cols ? cols[j] : j, c1 = cols ? cols[j + 1] : j + 1;
			idx[at++] = r0 + c0; idx[at++] = r1 + c0; idx[at++] = r0 + c1;
			idx[at++] = r0 + c1; idx[at++] = r1 + c0; idx[at++] = r1 + c1;
		}
	}
	return at;
}

// minimal TGA decoder: uncompressed and RLE true color
function decodeTga(buf) {
	const d = new Uint8Array(buf);
	const idLen = d[0], type = d[2], w = d[12] | (d[13] << 8), h = d[14] | (d[15] << 8), bpp = d[16], desc = d[17];
	if ((type !== 2 && type !== 10) || (bpp !== 24 && bpp !== 32)) return null;
	const px = bpp / 8, out = new Uint8Array(w * h * 4);
	let p = 18 + idLen + (d[1] ? (d[5] | (d[6] << 8)) * (d[7] >> 3) : 0), n = 0, alpha = false;
	const put = (o) => {
		out[n * 4] = d[o + 2]; out[n * 4 + 1] = d[o + 1]; out[n * 4 + 2] = d[o];
		out[n * 4 + 3] = px === 4 ? d[o + 3] : 255;
		if (px === 4 && d[o + 3] < 250) alpha = true;
		n++;
	};
	while (n < w * h && p < d.length) {
		if (type === 2) { put(p); p += px; continue; }
		const hdr = d[p++], count = (hdr & 127) + 1;
		if (hdr & 128) { for (let i = 0; i < count && n < w * h; i++) put(p); p += px; }
		else { for (let i = 0; i < count && n < w * h; i++) { put(p); p += px; } }
	}
	// TGA rows go bottom up unless bit 5 of the descriptor is set
	if (!(desc & 0x20)) {
		const row = w * 4, tmp = new Uint8Array(row);
		for (let y = 0; y < h >> 1; y++) {
			const a = y * row, b = (h - 1 - y) * row;
			tmp.set(out.subarray(a, a + row)); out.copyWithin(a, b, b + row); out.set(tmp, b);
		}
	}
	return { w, h, data: out, alpha };
}

