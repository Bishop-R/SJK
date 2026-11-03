// From VisionJK's web admin page (codemp/game/webadmin/page/js/scripts-parse.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
const SC_BLEND = {
	gl_one: gl.ONE, gl_zero: gl.ZERO, gl_dst_color: gl.DST_COLOR, gl_one_minus_dst_color: gl.ONE_MINUS_DST_COLOR,
	gl_src_color: gl.SRC_COLOR, gl_one_minus_src_color: gl.ONE_MINUS_SRC_COLOR, gl_src_alpha: gl.SRC_ALPHA,
	gl_one_minus_src_alpha: gl.ONE_MINUS_SRC_ALPHA, gl_dst_alpha: gl.DST_ALPHA, gl_one_minus_dst_alpha: gl.ONE_MINUS_DST_ALPHA,
	gl_src_alpha_saturate: gl.SRC_ALPHA_SATURATE
};
// shaderSort_t of the engine, by the words ParseSort knows. Without a word: 12 a shader that is only a fog
// (SS_FOG), 14 one that blends (SS_BLEND0), 15 one that adds (SS_BLEND1).
const SC_SORT = { portal: 1, sky: 2, opaque: 3, decal: 4, seethrough: 5, banner: 6, inside: 7, mid_inside: 8, middle: 9, mid_outside: 10, outside: 11, underwater: 13, additive: 15, nearest: 21 };
const SC_SORT_PORTAL = 1, SC_SORT_SKY = 2, SC_SORT_OPAQUE = 3, SC_SORT_DECAL = 4, SC_SORT_SEE_THROUGH = 5, SC_SORT_BANNER = 6, SC_SORT_FOG = 12, SC_SORT_BLEND = 14, SC_SORT_ADDITIVE = 15;
// Where the colour and the alpha of a stage come from (colorGen_t and alphaGen_t), by the words of a script.
// 0 is no rgbGen yet, and the alphaGen a stage begins with. skip is no word: the engine sets it.
const CGEN = { identity: 1, identitylighting: 2, entity: 3, oneminusentity: 4, exactvertex: 5, vertex: 6, oneminusvertex: 7, wave: 8, lightingdiffuse: 9, lightingdiffuseentity: 10, const: 11 };
const AGEN = { identity: 0, skip: 1, entity: 2, oneminusentity: 3, vertex: 4, oneminusvertex: 5, lightingspecular: 6, wave: 7, portal: 8, const: 9, dot: 10, oneminusdot: 11 };
// The words the engine reads that the page has nothing to do for: what is for the map compiler and the editor
// (and everything that begins with q3map or qer), the light of the sun, and how an image is kept (nomipmaps:
// read where the images are loaded, imageHasMips in js/map.js, nopicmip: the page makes no image smaller, notc: it
// compresses none).
// noglfog sets what GeneratePermanentShader then sets anew for every shader, so it does nothing in the engine.
const SC_NO_PICTURE = new Set(['material', 'sun', 'surfacelight', 'lightcolor', 'tesssize', 'clamptime', 'light', 'nomipmaps', 'nopicmip', 'notc', 'entitymergable', 'noglfog']);
// a material that is not of the map gets an order after all of the map (see prepareScripted)
const MAT_ORDER_LATE = 1 << 20;

// state: 0 not asked for, 1 on their way, 2 there, 3 the server has none to give (an older game module)
// dropped: the words of the scripts that were read and that the page does nothing with, and how often
const scripts = { state: 0, bodies: new Map(), defs: new Map(), materials: new Map(), dropped: new Map(), made: 0 };

const scNum = s => { const v = parseFloat(s); return Number.isFinite(v) ? v : 0; };
const scKey = name => name.toLowerCase().replace(/\\/g, '/').replace(/\.(tga|jpg|png)$/, '');

// The shaders of a .shader file: a name and what stands between its braces, kept as text until a map uses it.
// The first text of a name stays, as in the engine.
function scanShaderFile(text, into) {
	const re = /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"[^"\n]*"|[{}]|[^\s{}]+/g;
	let name = null, depth = 0, start = 0;
	for (let m; (m = re.exec(text)); ) {
		const t = m[0];
		if (t[0] === '/' && (t[1] === '/' || t[1] === '*')) continue;
		if (t === '{') {
			if (depth++ === 0) start = re.lastIndex;
		} else if (t === '}') {
			if (depth > 0 && --depth === 0 && name !== null) {
				const key = scKey(name);
				if (!into.has(key)) into.set(key, text.slice(start, m.index));
				name = null;
			}
		} else if (depth === 0) {
			name = t[0] === '"' ? t.slice(1, -1) : t;
		}
	}
}

async function loadShaderScripts() {
	if (scripts.state) return;
	scripts.state = 1;
	try {
		const res = await fetch('api/shaders', { cache: 'no-store' });
		if (!res.ok) throw new Error('HTTP ' + res.status);
		// the engine puts the files together last one first, so that is the one whose text of a shader counts
		const files = (await res.json()).files.slice().reverse(), texts = new Array(files.length);
		// The server does not answer for a moment while it changes the map. A file that could not be fetched then
		// is asked for again, a few rounds, before the map is drawn without its shaders.
		let todo = files.map((_, i) => i);
		for (let round = 0; todo.length && round < 6; round++) {
			if (round) await new Promise(done => setTimeout(done, 2500));
			const queue = todo;
			todo = [];
			const worker = async () => {
				while (queue.length) {
					const i = queue.shift();
					try {
						const file = await fetch('api/file?name=' + encodeURIComponent('shaders/' + files[i]));
						if (file.ok) texts[i] = await file.text(); else if (file.status !== 404) todo.push(i);
					} catch (e) {
						todo.push(i);
					}
				}
			};
			await Promise.all([1, 2, 3, 4, 5, 6].map(worker));
		}
		for (const text of texts) if (text) scanShaderFile(text, scripts.bodies);
		scripts.state = 2;
	} catch (e) {
		scripts.state = 3;
	}
	prepareScripted(world);
}

// the lines of a script as words, a brace is a line of its own
function scriptLines(body) {
	const lines = [], re = /\n|\/\/[^\n]*|\/\*[\s\S]*?\*\/|"[^"\n]*"|[{}]|[^\s{}]+/g;
	let cur = null;
	for (let m; (m = re.exec(body)); ) {
		const t = m[0];
		if (t === '\n') { cur = null; continue; }
		if (t[0] === '/' && (t[1] === '/' || t[1] === '*')) { if (t.includes('\n')) cur = null; continue; }
		if (t === '{' || t === '}') { lines.push([t]); cur = null; continue; }
		if (!cur) lines.push(cur = []);
		cur.push(t[0] === '"' ? t.slice(1, -1) : t);
	}
	return lines;
}

function scWave(a, i) {
	return { func: (a[i] || 'sin').toLowerCase(), base: scNum(a[i + 1]), amp: scNum(a[i + 2]), phase: scNum(a[i + 3]), freq: scNum(a[i + 4]) };
}

function waveValue(w, t) {
	const x = w.phase + t * w.freq, f = x - Math.floor(x);
	let v = 1;
	if (w.func === 'sin') v = Math.sin(x * Math.PI * 2);
	else if (w.func === 'triangle') v = f < 0.25 ? f * 4 : f < 0.75 ? 2 - f * 4 : f * 4 - 4;
	else if (w.func === 'square') v = f < 0.5 ? 1 : -1;
	else if (w.func === 'sawtooth') v = f;
	else if (w.func === 'inversesawtooth') v = 1 - f;
	else if (w.func === 'noise') v = Math.random() * 2 - 1;
	return w.base + w.amp * v;
}

// a word of a script that the page does nothing with
function scDrop(what) {
	scripts.dropped.set(what, (scripts.dropped.get(what) || 0) + 1);
}

// ParseVector: ( x y z ), each a word of its own. null: the brackets are not there.
function scVector(a, i) {
	return a[i] === '(' && a[i + 4] === ')' ? [scNum(a[i + 1]), scNum(a[i + 2]), scNum(a[i + 3])] : null;
}

function newStage() {
	return { maps: [], images: null, tex: null, animFreq: 0, oneShot: false, clamp: false, lightmap: false, isLightmap: false, white: false, skip: false,
		src: -1, dst: -1, blend: null, alphaFunc: 0, depthEqual: false, noDepth: false, depthWrite: true, depthWriteAsked: false,
		rgbGen: 0, rgbWave: null, rgbConst: null, alphaGen: AGEN.identity, alphaWave: null, alphaConst: 0, portalRange: 0,
		tcGen: 0, tcGenAsked: false, tcMods: [],
		// what FinishShader makes of the stage (finishMaterial)
		rgbMode: 0, alphaMode: 0, adjustFog: 0, fogOverride: 0, fogPass: false, multi: null, env: 0, merged: false };
}

// a line of a stage (ParseStage)
function stageLine(st, k, a, def) {
	const v = (a[1] || '').toLowerCase();
	if (k === 'map' || k === 'clampmap') {
		if (v === '$lightmap') st.lightmap = st.isLightmap = true;
		else if (v === '$whiteimage' || v === '*white') st.white = true;
		else if (a[1]) st.maps = [a[1]];
		st.clamp = k === 'clampmap';
	} else if (k === 'animmap' || k === 'clampanimmap' || k === 'oneshotanimmap') {
		st.animFreq = scNum(a[1]);
		st.maps = a.slice(2);
		st.clamp = k === 'clampanimmap';
		// it runs through its images once and stays on the last: an explosion
		st.oneShot = k === 'oneshotanimmap';
	} else if (k === 'videomap' || k === 'surfacesprites') {
		st.skip = true;
	} else if (k === 'blendfunc') {
		if (!v) scDrop('blendfunc without a word');
		else if (v === 'add') { st.src = gl.ONE; st.dst = gl.ONE; }
		else if (v === 'filter') { st.src = gl.DST_COLOR; st.dst = gl.ZERO; }
		else if (v === 'blend') { st.src = gl.SRC_ALPHA; st.dst = gl.ONE_MINUS_SRC_ALPHA; }
		else { st.src = SC_BLEND[v] ?? gl.ONE; st.dst = SC_BLEND[(a[2] || '').toLowerCase()] ?? gl.ONE; }
	} else if (k === 'alphafunc') {
		st.alphaFunc = { gt0: 1, lt128: 2, ge128: 3, ge192: 4 }[v] || 0;
	} else if (k === 'depthfunc') {
		if (v !== 'lequal' && v !== 'equal' && v !== 'disable') { scDrop('depthfunc ' + v); return; }
		st.depthEqual = v === 'equal';
		st.noDepth = v === 'disable';
	} else if (k === 'depthwrite') {
		st.depthWriteAsked = true;
	} else if (k === 'rgbgen') {
		const g = CGEN[v];
		if (typeof g !== 'number') { scDrop('rgbgen ' + v); return; }
		st.rgbGen = g;
		if (g === CGEN.wave) st.rgbWave = scWave(a, 2);
		else if (g === CGEN.const) st.rgbConst = scVector(a, 2) || [0, 0, 0];
		// the alpha of the vertex comes along unless an alphaGen stood before it
		else if (g === CGEN.vertex && st.alphaGen === AGEN.identity) st.alphaGen = AGEN.vertex;
	} else if (k === 'alphagen') {
		const g = AGEN[v];
		if (typeof g !== 'number' || v === 'skip') { scDrop('alphagen ' + v); return; }
		st.alphaGen = g;
		if (g === AGEN.wave) st.alphaWave = scWave(a, 2);
		else if (g === AGEN.const) st.alphaConst = scNum(a[2]);
		// the range is one of the shader (0 while no stage has named one), the last stage that names one sets it
		else if (g === AGEN.portal) def.portalRange = a[2] ? scNum(a[2]) : 256;
	} else if (k === 'tcgen' || k === 'texgen') {
		if (v !== 'lightmap' && v !== 'environment' && v !== 'texture' && v !== 'base') { scDrop('tcgen ' + v); return; }
		st.tcGen = v === 'lightmap' ? 1 : v === 'environment' ? 2 : 0;
		st.tcGenAsked = true;
	} else if (k === 'tcmod') {
		const kind = v, n = a.slice(2).map(scNum);
		if (kind === 'stretch') st.tcMods.push({ kind, wave: scWave(a, 2) });
		else if (kind === 'scroll' || kind === 'scale' || kind === 'rotate' || kind === 'transform' || kind === 'turb') st.tcMods.push({ kind, n });
		// entityTranslate moves by what the entity says, which is nothing for all the page draws
		else if (kind !== 'entitytranslate') scDrop('tcmod ' + kind);
	} else if (k !== 'detail' && k !== 'glow' && !k.startsWith('ss')) {
		// detail: drawn like any stage (r_detailTextures is on). glow: only with r_DynamicGlow. ss...: what the
		// sprites of a surface are like, which are not drawn.
		scDrop('stage ' + k);
	}
}

// what the engine settles when a stage is read to its end (ParseStage)
function finishStage(st) {
	// without an rgbGen: identityLighting where the stage adds or blends by its alpha, identity where not
	if (!st.rgbGen) st.rgbGen = st.src === gl.ONE || st.src === gl.SRC_ALPHA ? CGEN.identitylighting : CGEN.identity;
	// one and zero is no blending. A stage that blends leaves the depth alone unless it says otherwise.
	st.blend = st.src < 0 || (st.src === gl.ONE && st.dst === gl.ZERO) ? null : [st.src, st.dst];
	st.depthWrite = !st.blend || st.depthWriteAsked;
	if (st.alphaGen === AGEN.identity && (st.rgbGen === CGEN.identity || st.rgbGen === CGEN.lightingdiffuse)) st.alphaGen = AGEN.skip;
	if (st.lightmap && !st.tcGenAsked) st.tcGen = 1;
	return st;
}

// a line of a shader outside of its stages (ParseShader)
function shaderLine(def, k, a) {
	const v = (a[1] || '').toLowerCase();
	if (k === 'surfaceparm') {
		// forcesight: only seen with the sight of the Force, which the page never has (R_AddDrawSurf)
		if (v === 'nodraw' || v === 'forcesight') def.nodraw = true;
		else if (v === 'fog') def.contentsFog = true;
	}
	else if (k === 'skyparms') def.sky = { box: a[1] && a[1] !== '-' ? a[1] : '', height: scNum(a[2]) || 512 };
	else if (k === 'sort') def.sort = SC_SORT[v] || scNum(a[1]);
	else if (k === 'portal') def.sort = SC_SORT_PORTAL;
	else if (k === 'polygonoffset') def.polygonOffset = true;
	else if (k === 'cull') {
		// which side is drawn: the front unless the script says the back or both
		if (v === 'none' || v === 'twosided' || v === 'disable') def.cull = 2;
		else if (v === 'back' || v === 'backside' || v === 'backsided') def.cull = 1;
		else scDrop('cull ' + v);
	}
	else if (k === 'deformvertexes' || k === 'deform') {
		const div = scNum(a[2]);
		// a wave over a length of 0 is not allowed, the engine takes this for it
		if (v === 'wave') def.deforms.push({ kind: v, spread: div ? 1 / div : 100, wave: scWave(a, 3) });
		else if (v === 'bulge') def.deforms.push({ kind: v, width: scNum(a[2]), height: scNum(a[3]), speed: scNum(a[4]) });
		else if (v === 'move') def.deforms.push({ kind: v, vec: [scNum(a[2]), scNum(a[3]), scNum(a[4])], wave: scWave(a, 5) });
		else if (v === 'autosprite' || v === 'autosprite2') def.deforms.push({ kind: v });
		else scDrop('deformvertexes ' + v);
	}
	else if (k === 'fogparms') {
		// ( r g b ) and the depth at which nothing is seen through it any more
		const color = scVector(a, 1);
		if (color) def.fog = { color, depth: scNum(a[6]) }; else scDrop('fogparms without brackets');
	}
	else if (!SC_NO_PICTURE.has(k) && !k.startsWith('q3map') && !k.startsWith('qer')) scDrop('shader ' + k);
}

function parseScript(body) {
	const def = { stages: [], sort: 0, polygonOffset: false, nodraw: false, sky: null, fog: null, contentsFog: false, cull: 0, deforms: [], portalRange: 0 };
	let st = null, depth = 0;
	for (const a of scriptLines(body)) {
		const k = a[0].toLowerCase();
		if (k === '{') {
			if (depth++ === 0) st = newStage();
		} else if (k === '}') {
			if (depth > 0 && --depth === 0 && st) {
				if (!st.skip) def.stages.push(finishStage(st));
				st = null;
			}
		} else if (depth === 1 && st) {
			stageLine(st, k, a, def);
		} else if (depth === 0) {
			shaderLine(def, k, a);
		}
	}
	return def;
}

// The pairs of blend functions CollapseMultitexture knows: 1 and 2 multiply with what is there, 3 adds to it,
// 0 does not blend, -1 any other.
function blendKind(b) {
	return !b ? 0 : b[0] === gl.DST_COLOR && b[1] === gl.ZERO ? 1 : b[0] === gl.ZERO && b[1] === gl.SRC_COLOR ? 2 : b[0] === gl.ONE && b[1] === gl.ONE ? 3 : -1;
}
const sameWave = (a, b) => a.func === b.func && a.base === b.base && a.amp === b.amp && a.phase === b.phase && a.freq === b.freq;

// What FinishShader and GeneratePermanentShader settle once a shader has its stages: its sort, how each stage
// takes a fog, and the first two stages made one where the engine draws them with two images at once.
function finishMaterial(mat, def) {
	const stages = mat.stages, first = stages[0];
	let sort = mat.sky ? SC_SORT_SKY : (def && def.sort) || (mat.polygonOffset ? SC_SORT_DECAL : 0);
	stages.forEach((st, i) => {
		const b = st.blend, src = b ? b[0] : -1, dst = b ? b[1] : -1, add = src === gl.ONE && dst === gl.ONE;
		st.adjustFog = 0;
		if (b && first.blend) {
			// A stage that blends, on a shader whose first stage blends, fades as a fog gets thicker: 1 its colour,
			// 2 colour and alpha, 3 its alpha. Other ways of blending can not be faded and stay as they are.
			st.adjustFog = add || (src === gl.ZERO && dst === gl.ONE_MINUS_SRC_COLOR) ? 1 : src === gl.SRC_ALPHA && dst === gl.ONE_MINUS_SRC_ALPHA ? 3
				: src === gl.ONE && dst === gl.ONE_MINUS_SRC_ALPHA ? 2 : 0;
			// and the first of them says where the shader is in the order of drawing
			if (!sort) sort = st.depthWrite ? SC_SORT_SEE_THROUGH : add ? SC_SORT_ADDITIVE : SC_SORT_BLEND;
		}
		// What the fog of OpenGL fades the stage to (mGLFogColorOverride): 0 the colour of the fog, 1 black for
		// what adds to the picture, 2 white for what multiplies it. The table is the engine's, case by case.
		st.fogOverride = !b ? (i || (st.isLightmap && stages[i + 1] && stages[i + 1].isLightmap) ? 2 : 0)
			: add ? 1
			: src === gl.SRC_ALPHA && dst === gl.ONE && st.alphaGen === AGEN.lightingspecular && i ? 1
			: src === gl.ZERO && dst === gl.ZERO ? 2
			: src === gl.DST_COLOR && dst === gl.ZERO ? (st.isLightmap ? 2 : 1)
			: src === gl.ONE && dst === gl.ONE_MINUS_SRC_COLOR ? 1 : 0;
		// what of the colour the vertices carry (ComputeColors): 1 their colour, 2 one minus it, 3 the light of the place
		const cg = st.rgbGen, ag = st.alphaGen;
		st.rgbMode = cg === CGEN.vertex || cg === CGEN.exactvertex ? 1 : cg === CGEN.oneminusvertex ? 2 : cg === CGEN.lightingdiffuse || cg === CGEN.lightingdiffuseentity ? 3 : 0;
		// and of the alpha: 1 theirs (exactVertex copies it, and after rgbGen vertex an alphaGen identity leaves
		// it), 2 one minus it, 3 the shine, 4 how far from the eye
		st.alphaMode = ag === AGEN.vertex || (ag === AGEN.skip && cg === CGEN.exactvertex) || (ag === AGEN.identity && cg === CGEN.vertex) ? 1
			: ag === AGEN.oneminusvertex ? 2 : ag === AGEN.lightingspecular ? 3 : ag === AGEN.portal ? 4 : 0;
		st.portalRange = mat.portalRange;
	});

	// CollapseMultitexture: two stages that only differ in how they blend, and blend in one of eight ways, are
	// drawn as one with two images (env 1 multiplied, 2 added). The second one stays in the list, as merged.
	const second = stages[1], ka = blendKind(first && first.blend), kb = blendKind(second && second.blend);
	const env = !second || ka < 0 || kb <= 0 ? 0 : kb === 3 ? (ka === 0 || ka === 3 ? 2 : 0) : ka === 3 ? 0 : 1;
	if (env && first.alphaFunc === second.alphaFunc && first.depthEqual === second.depthEqual && first.noDepth === second.noDepth
		&& first.rgbGen === second.rgbGen && first.alphaGen === second.alphaGen && (env === 1 || first.rgbGen === CGEN.identity)
		&& (first.rgbGen !== CGEN.wave || sameWave(first.rgbWave, second.rgbWave)) && (first.alphaGen !== AGEN.wave || sameWave(first.alphaWave, second.alphaWave))) {
		first.multi = second;
		first.env = env;
		first.blend = ka === 0 ? null : env === 2 ? [gl.ONE, gl.ONE] : [gl.DST_COLOR, gl.ZERO];
		second.merged = true;
	}

	// a shader without a stage that is no sky is the fog of a brush and nothing else
	mat.sort = !stages.length && !mat.sky ? SC_SORT_FOG : sort || SC_SORT_OPAQUE;
	// Whether a fog is drawn onto its surfaces: 1 where they were drawn (FP_EQUAL), 2 over what is behind them
	// (FP_LE, the surface of a fog itself), 0 not: what blends fades in a fog instead (adjustFog).
	mat.fogPass = mat.sort <= SC_SORT_SEE_THROUGH ? 1 : mat.contentsFog ? 2 : 0;
	for (const st of stages) st.fogPass = mat.fogPass > 0;
}

// The stages a surface of the map is drawn with: the script of its shader, or what the engine makes of an image
// that has none (R_FindShader). A surface without a lightmap is lit by the colours of its vertices, with the rules
// of the engine for that (FinishShader). lit: true for a surface with a lightmap, false for one without, 'model'
// for a model, which gets the light of the place where a shader has no script, 'fx' for a primitive of an
// effect, which has the colour and the alpha of its vertices there and is blended.
function materialFor(name, lit) {
	const key = scKey(name), mkey = key + (lit === 'model' ? '|m' : lit === 'fx' ? '|f' : lit ? '|l' : '|v');
	let mat = scripts.materials.get(mkey);
	if (mat) return mat;

	let def = scripts.defs.get(key);
	if (def === undefined) {
		const body = scripts.bodies.get(key);
		def = body === undefined ? null : parseScript(body);
		scripts.defs.set(key, def);
	}

	let stages;
	if (!def) {
		const image = Object.assign(newStage(), name ? { maps: [name] } : { white: true });
		if (lit === 'model') {
			image.rgbGen = CGEN.lightingdiffuse;
			stages = [image];
		} else if (lit === 'fx') {
			Object.assign(image, { rgbGen: CGEN.vertex, alphaGen: AGEN.vertex, src: gl.SRC_ALPHA, dst: gl.ONE_MINUS_SRC_ALPHA });
			stages = [image];
		} else if (lit) {
			Object.assign(image, { rgbGen: CGEN.identity, src: gl.DST_COLOR, dst: gl.ZERO });
			stages = [Object.assign(newStage(), { lightmap: true, isLightmap: true, rgbGen: CGEN.identity }), image];
		} else {
			Object.assign(image, { rgbGen: CGEN.exactvertex, alphaGen: AGEN.skip });
			stages = [image];
		}
		stages.forEach(finishStage);
	} else {
		stages = def.stages.map(st => Object.assign({}, st));
		const lm = lit ? -1 : stages.findIndex(st => st.isLightmap);
		if (lit === 'model') {
			// a model has no lightmap: such a stage is white
			for (const st of stages) if (st.lightmap) Object.assign(st, { lightmap: false, white: true });
		} else if (lm === 0 && stages.length > 1) {
			// the stage after the lightmap takes its place, as the first thing that is drawn there
			stages.shift();
			Object.assign(stages[0], { blend: null, depthWrite: true, alphaFunc: 0, depthEqual: false, noDepth: false, rgbGen: CGEN.exactvertex, alphaGen: AGEN.skip });
		} else if (lm >= 0) {
			Object.assign(stages[lm], { lightmap: false, white: true, rgbGen: CGEN.exactvertex, alphaGen: AGEN.skip });
		}
	}

	mat = {
		stages,
		nodraw: !!def && def.nodraw,
		polygonOffset: !!def && def.polygonOffset,
		sky: def ? def.sky : null,
		fog: def ? def.fog : null,
		contentsFog: !!def && def.contentsFog,
		cull: def ? def.cull : 0,
		deforms: def ? def.deforms : [],
		portalRange: def ? def.portalRange : 0,
		sort: 0, fogPass: 0,
		// A number that rises with the order in which the engine registers its shaders, which is what it sorts by
		// where two have the same sort (SortNewShader): for a shader of the map the place of the first surface
		// that has it (prepareScripted). The others come after the map, in the order the page first asked for them.
		order: MAT_ORDER_LATE + scripts.made++
	};
	finishMaterial(mat, def);
	// An effect never writes the depth, whatever its script says: what is drawn after it (the blades, the weapon
	// in front of the eye) is not cut out by a quad of smoke.
	if (lit === 'fx') for (const st of stages) st.depthWrite = false;
	scripts.materials.set(mkey, mat);
	return mat;
}

// The image a map editor shows for a shader, for the plain view: the one its script names for that
// (qer_editorimage), or the first one of its stages. Its name, '' for none: no script, or the scripts are not
// there yet.
function editorImageName(name) {
	const body = scripts.state === 2 ? scripts.bodies.get(scKey(name)) : undefined;
	if (body === undefined) return '';
	const named = /^\s*qer_editorimage\s+(\S+)/im.exec(body);
	const stage = named ? null : materialFor(name, true).stages.find(st => st.maps.length);
	return named ? named[1] : stage ? stage.maps[0] : '';
}
function editorImage(name) {
	const image = editorImageName(name);
	return image ? gameImage(image, false) : null;
}

// the images the stages name, a few on their way at a time
const gameImages = new Map();	// name and whether it is clamped -> { tex, alpha, ready, failed }, of the map that is shown
const imageQueue = [];
let imageBusy = 0;

function gameImage(name, clamp) {
	const key = name.toLowerCase() + (clamp ? '|c' : '');
	let h = gameImages.get(key);
	if (!h) {
		h = { tex: grayTex, alpha: false, ready: false, failed: false };
		gameImages.set(key, h);
		imageQueue.push([h, name, clamp]);
		pumpImages();
	}
	return h;
}

function pumpImages() {
	while (imageBusy < 6 && imageQueue.length) {
		const [h, name, clamp] = imageQueue.shift();
		imageBusy++;
		fetchImage(name, clamp).then(img => {
			if (img) { h.tex = img.tex; h.alpha = img.alpha; h.ready = true; } else h.failed = true;
		}, () => {
			// no answer: again in a moment, three times, if the image is still one of the map that is shown
			h.tries = (h.tries || 0) + 1;
			if (h.tries > 3) { h.failed = true; return; }
			setTimeout(() => { if (gameImages.get(name.toLowerCase() + (clamp ? '|c' : '')) === h) { imageQueue.push([h, name, clamp]); pumpImages(); } }, 2500);
		}).finally(() => { imageBusy--; pumpImages(); });
	}
}

