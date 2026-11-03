// From VisionJK's web admin page (codemp/game/webadmin/page/js/editor-remaps.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: cut: only its first part (the swaps the view draws, remapApply, and the pictures of the shaders); the pane, the marks and the server's swaps are left out.
// ---- the texture swap: a shader of the map drawn as another one, for every player (/api/remaps, /api/remap)
//
// The game can draw a shader as another one while the map runs: the server keeps a table of names and tells it
// to every game, also to one that joins later, and each game swaps the shader by itself. The unchanged game
// does that too, it is how a button of a map changes a screen. This tab fills that table.
//
// What it can not do: swap one wall. A shader is swapped wherever the map has it. And a game only draws a
// shader it has, one of the game itself or of a pk3 it has: a name it does not know is left as it is there.
//
// A wall that is lit by a lightmap keeps it in a game of this tree. Another game draws it without one once it
// is swapped, and keeps that after the swap is taken back, until the map is loaded again.
//
// The swaps are kept by Save in a file of the map and come back with it. What the map swaps by itself is
// shown here and can be taken back, and is not saved.
//
// The pane has a small picture of the selected shader, of what it would be drawn as and of every row. The map
// marks the surfaces that have the selected shader, as the Clips tab marks a clip. What to draw it as is chosen
// from the pictures of all textures, and a test object shows it with its whole shader (js/editor-textures.js).
//
// A texture that is chosen from the pictures is a draft first, as a new entity of the World tab is: the view of
// this page draws the shader as it at once, and the server and the players know nothing of it until Swap sends
// it.

// list: { old, new, own } as the server has them. own: made here or read from the file of the map, not by an
// entity of the map. to: the same for the view, by the key of the old name. selected: the shader the pane
// tells of. used, budget: the characters the swaps take of what a game is told when it connects, told and
// toldRoom: all it is told, and how much of that the swaps may bring it to. seen: the
// map the view has the swaps for. uses: the shaders of that map. mark: the surfaces of the selected shader as
// they are drawn over the map (remapMark). shown: the shader the test object is drawn with. drafts: the swaps
// that are only in this page, { old, new } by the key of the old name.
const remap = { list: [], to: new Map(), dirty: false, used: 0, budget: 0, known: true, key: '', busy: false, selected: '', seen: null, uses: null, usesOf: null, namesOf: null, namesState: -1, mark: null, shown: '', drafts: new Map(), told: 0, toldRoom: 0, last: null };
const REMAP_ROWS = 150;

// what a shader of the map is drawn as
const remapOf = name => remap.to.get(scKey(name)) || name;
const remapNow = name => remap.list.find(r => scKey(r.old) === scKey(name));

// The shaders the surfaces of the map have, made once for a map: name -> { surfaces, lit }. lit: one of them
// has a lightmap.
function remapUses() {
	if (!world) return new Map();
	if (remap.uses && remap.usesOf === world) return remap.uses;
	const uses = new Map();
	for (const sf of world.surfs) {
		const sh = sf && world.shaders[sf.shader];
		if (!sh) continue;
		let u = uses.get(sh.name);
		if (!u) uses.set(sh.name, u = { surfaces: 0, lit: false });
		u.surfaces++;
		if (sf.lm >= 0 && sf.lm < world.stats.lightmaps) u.lit = true;
	}
	remap.usesOf = world;
	return remap.uses = uses;
}

// The view draws the swaps too (js/frame.js, js/scripts-world.js), and in the editor a draft as if it were one:
// the administration shows the map as the players have it. The image of a new name is asked for as the ones of
// the map are, and what the scripts draw is put together again. Not the sky: the view keeps the one of the map.
function remapApply() {
	remap.to = new Map(remap.list.map(r => [scKey(r.old), r.new]));
	if (appMode === 'editor') for (const [key, d] of remap.drafts) remap.to.set(key, d.new);
	remap.seen = world;
	if (!world) return;
	for (const name of remap.to.values()) if (!world.textures.has(name)) world.textures.set(name, gameImage(name, false));
	prepareScripted(world);
}

// ---- the pictures of the shaders, small ones for the pane
//
// The picture of a shader is the image of its name, or else the one a map editor shows for it (editorImageName):
// what the plain view of the editor draws on a wall, not what the stages of a script make of it.

const THUMB_SIZE = 128;
// the key of a shader -> { state: 0 on its way, 1 there, 2 none, url, file, w, h, blind: asked before the
// scripts were read }
const thumbs = new Map();
const thumbQueue = [];
let thumbBusy = 0;

// the file of an image as a small picture: the address the page has it at, '' when it could not be made
async function thumbDraw(f) {
	const k = Math.min(1, THUMB_SIZE / Math.max(f.w, f.h)), cv = document.createElement('canvas'), g = cv.getContext('2d');
	let src = f.img;
	cv.width = Math.max(1, Math.round(f.w * k));
	cv.height = Math.max(1, Math.round(f.h * k));
	if (f.tga) {
		// without its alpha, as the game draws an image that has no script
		const full = document.createElement('canvas'), px = new Uint8ClampedArray(f.img.data);
		for (let i = 3; i < px.length; i += 4) px[i] = 255;
		full.width = f.w; full.height = f.h;
		full.getContext('2d').putImageData(new ImageData(px, f.w, f.h), 0, 0);
		src = full;
	}
	g.imageSmoothingQuality = 'high';
	g.drawImage(src, 0, 0, cv.width, cv.height);
	if (!f.tga) f.img.close();
	const blob = await new Promise(done => cv.toBlob(done, 'image/jpeg', 0.85));
	return blob ? URL.createObjectURL(blob) : '';
}

async function thumbMake(name) {
	const images = [name, editorImageName(name)].filter((n, i, all) => n && all.findIndex(o => scKey(o) === scKey(n)) === i);
	for (const image of images) {
		const f = await fetchImageFile(image);
		if (!f || !f.w || !f.h) continue;
		let url = '';
		try { url = await thumbDraw(f); } catch (e) { /* an image the page can not draw: the next one */ }
		if (url) return { state: 1, url, file: f.file, w: f.w, h: f.h };
	}
	return { state: 2 };
}

// a few on their way at a time
function thumbPump() {
	while (thumbBusy < 3 && thumbQueue.length) {
		const [t, name] = thumbQueue.shift(), key = scKey(name);
		thumbBusy++;
		thumbMake(name).then(made => Object.assign(t, made), () => {
			// the server did not answer: asked again when it is shown again
			if (thumbs.get(key) === t) thumbs.delete(key);
		}).finally(() => {
			thumbBusy--;
			for (const el of document.querySelectorAll('[data-thumb]')) if (scKey(el.dataset.thumb) === key) thumbShow(el, t);
			thumbPump();
		});
	}
}

// The picture of a shader. ask false: only when it was asked for before, else null.
function thumbOf(name, ask = true) {
	const key = scKey(name);
	let t = thumbs.get(key);
	// none while the scripts were not read: its script may name an image
	if (t && t.state === 2 && t.blind && scripts.state === 2) t = undefined;
	if (!t && ask) {
		thumbs.set(key, t = { state: 0, url: '', file: '', w: 0, h: 0, blind: scripts.state !== 2 });
		thumbQueue.push([t, name]);
		thumbPump();
	}
	return t || null;
}

// An element of the page that names a shader (data-thumb) shows the picture of it. Its title, and a caption
// right after it, say which file that is.
function thumbShow(el, t) {
	const there = !!t && t.state === 1, none = !!t && t.state === 2, cap = el.nextElementSibling;
	const what = there ? t.file + ', ' + t.w + ' x ' + t.h : !none ? '' :
		scripts.bodies.has(scKey(el.dataset.thumb)) ? 'a script without an image to show' : 'no image of that name';
	el.style.backgroundImage = there ? 'url(' + t.url + ')' : '';
	el.classList.toggle('none', none);
	el.title = what;
	if (cap && cap.tagName === 'FIGCAPTION') cap.textContent = what;
}
function thumbFill(el) {
	thumbShow(el, el.dataset.thumb ? thumbOf(el.dataset.thumb) : null);
}

// What a list shows asks for its pictures when it comes into view. A list is watched anew when it is built
// again: the ones it had before are let go.
const thumbSeen = new IntersectionObserver(seen => {
	for (const s of seen) if (s.isIntersecting) { thumbSeen.unobserve(s.target); thumbFill(s.target); }
}, { rootMargin: '120px' });
const thumbWaiting = new WeakMap();
function thumbWatch(box) {
	const waiting = [];
	for (const el of thumbWaiting.get(box) || []) thumbSeen.unobserve(el);
	for (const el of box.querySelectorAll('[data-thumb]')) {
		const t = thumbOf(el.dataset.thumb, false);
		if (t && t.state) thumbShow(el, t); else { thumbSeen.observe(el); waiting.push(el); }
	}
	thumbWaiting.set(box, waiting);
}

