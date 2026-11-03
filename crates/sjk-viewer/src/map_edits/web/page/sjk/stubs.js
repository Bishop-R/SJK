// ---- what VisionJK's drawing asks of the parts of its panel this page leaves out
//
// After VisionJK's js/live-stubs.js: the drawing asks the server below api/, and some of its files read the
// state of a game server (players, movers, items, the first person view). SJK's map editor draws the map
// alone, the way the map file has it, so each of those is what it is when there is nothing of the kind.

// The page's address carries its token (?t=); every question to SJK carries it as a header.
const SJK_TOKEN = new URLSearchParams(location.search).get('t') || '';
const fetch = (url, opts = {}) => {
	const headers = new Headers(opts.headers || {});
	headers.set('X-SJK-Token', SJK_TOKEN);
	return window.fetch(url, Object.assign({}, opts, { headers }));
};

// what the server's state would say: no game runs on this page (the gametype of the static models, 0: ffa)
let state = { map: '', gametype: 0 };
const AT_REST = [null];
const movers = null;
function moverPlaces() { return AT_REST; }
function eyeAt(place) {
	const at = M4.transform(M4.invert(place), eye);
	return [at[0], at[1], at[2]];
}
const players = new Map();
const view = { id: -1, yaw: 0, pitch: 0 };
const appMode = 'admin';
let frameNow = 0, lastFrame = 0;
function dlightsOn() {}
function pollState() {}
const canvasScale = () => Math.min(window.devicePixelRatio || 1, 2);
function resize() {
	const dpr = canvasScale();
	canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr));
	canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr));
}

// SJK's map edits as the drawing reads them: the BSP surfaces not drawn (js/map.js keeps a hidden patch hidden)
// and the map's misc_model_static entities not drawn, by their number (js/world-ents.js)
const sjkHidden = new Set();
const sjkHiddenProps = new Set();
