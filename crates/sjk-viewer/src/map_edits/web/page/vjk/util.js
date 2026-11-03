// From VisionJK's web admin page (codemp/game/webadmin/page/js/util.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
'use strict';

/* ------------------------------------------------------------------ */
/* helpers                                                             */
/* ------------------------------------------------------------------ */

// The live view of a website (live.js, js/live-boot.js) is the drawing of this page without the rest of it: an
// element it does not have is a detached one, so what the panel would show there goes nowhere.
const LIVE = !!window.VJK_LIVE;
const stubElements = new Map();
const $ = id => document.getElementById(id) || stubElement(id);
function stubElement(id) {
	let e = stubElements.get(id);
	if (!e) { e = document.createElement('div'); stubElements.set(id, e); }
	return e;
}
const WEAPONS = ['None','Stun baton','Melee','Saber','Bryar pistol','Blaster','Disruptor','Bowcaster','Repeater','DEMP 2','Flechette','Rocket launcher','Thermal','Trip mine','Det pack','Concussion','Bryar old','Emplaced gun','Turret'];
const ITEM_COLORS = { 1: [1, .8, .2], 2: [.8, .6, .2], 3: [.35, .65, 1], 4: [.3, 1, .4], 5: [1, .3, 1], 6: [.9, .9, .9], 7: [1, .3, 1], 8: [1, 1, 1] };
const TEAM_COLORS = { red: [.9, .25, .28], blue: [.25, .55, .97], free: [.96, .85, .1], spectator: [.5, .5, .55] };

function escapeHtml(s) {
	return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

// Quake color codes (^1red) to HTML
function colorName(name) {
	let out = '', color = 7, chunk = '';
	const flush = () => { if (chunk) out += '<span class="c' + color + '">' + escapeHtml(chunk) + '</span>'; chunk = ''; };
	for (let i = 0; i < name.length; i++) {
		if (name[i] === '^' && i + 1 < name.length && name[i + 1] >= '0' && name[i + 1] <= '9') {
			flush();
			color = +name[i + 1];
			i++;
		} else {
			chunk += name[i];
		}
	}
	flush();
	return out;
}

// A session ends after half an hour without a request, and at the latest after twelve hours: the server then
// answers 401, and the page is loaded again, which is the page of the login.
{
	const plainFetch = window.fetch.bind(window);
	let leaving = false;
	window.fetch = async (...args) => {
		const res = await plainFetch(...args);
		if (res.status === 401 && !leaving) { leaving = true; location.reload(); }
		return res;
	};
}

function plainName(name) { return name.replace(/\^[0-9]/g, ''); }

function toast(msg, ok = true) {
	const t = document.createElement('div');
	t.className = 'toast' + (ok ? '' : ' err');
	t.textContent = msg;
	$('toasts').appendChild(t);
	setTimeout(() => t.remove(), 3500);
}

// A POST to the server: fields as a form, or a text as it is. The answer as an object, and { ok: false,
// message } when there is none. missing: what to say when the server does not know the address (404).
async function apiPost(path, fields, missing) {
	try {
		const res = await fetch(path, { method: 'POST', body: typeof fields === 'string' ? fields : new URLSearchParams(fields || {}) });
		if (res.status === 404 && missing) return { ok: false, message: missing };
		return await res.json();
	} catch (e) {
		return { ok: false, message: 'Request failed: ' + e.message };
	}
}

async function action(params) {
	const json = await apiPost('api/action', params);
	toast(json.message, json.ok);
	pollState();
	return json.ok;
}

/* ------------------------------------------------------------------ */
/* matrices (column major, like GL)                                    */
/* ------------------------------------------------------------------ */

const M4 = {
	perspective(fovy, aspect, near, far) {
		const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
		return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
	},
	lookAt(eye, at, up) {
		let zx = eye[0] - at[0], zy = eye[1] - at[1], zz = eye[2] - at[2];
		let l = Math.hypot(zx, zy, zz); zx /= l; zy /= l; zz /= l;
		let xx = up[1] * zz - up[2] * zy, xy = up[2] * zx - up[0] * zz, xz = up[0] * zy - up[1] * zx;
		l = Math.hypot(xx, xy, xz); xx /= l; xy /= l; xz /= l;
		const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
		return new Float32Array([xx, yx, zx, 0, xy, yy, zy, 0, xz, yz, zz, 0,
			-(xx * eye[0] + xy * eye[1] + xz * eye[2]), -(yx * eye[0] + yy * eye[1] + yz * eye[2]), -(zx * eye[0] + zy * eye[1] + zz * eye[2]), 1]);
	},
	mul(a, b) {
		const r = new Float32Array(16);
		for (let c = 0; c < 4; c++)
			for (let row = 0; row < 4; row++)
				r[c * 4 + row] = a[row] * b[c * 4] + a[4 + row] * b[c * 4 + 1] + a[8 + row] * b[c * 4 + 2] + a[12 + row] * b[c * 4 + 3];
		return r;
	},
	invert(m) {
		const inv = new Float32Array(16);
		inv[0] = m[5]*m[10]*m[15]-m[5]*m[11]*m[14]-m[9]*m[6]*m[15]+m[9]*m[7]*m[14]+m[13]*m[6]*m[11]-m[13]*m[7]*m[10];
		inv[4] = -m[4]*m[10]*m[15]+m[4]*m[11]*m[14]+m[8]*m[6]*m[15]-m[8]*m[7]*m[14]-m[12]*m[6]*m[11]+m[12]*m[7]*m[10];
		inv[8] = m[4]*m[9]*m[15]-m[4]*m[11]*m[13]-m[8]*m[5]*m[15]+m[8]*m[7]*m[13]+m[12]*m[5]*m[11]-m[12]*m[7]*m[9];
		inv[12] = -m[4]*m[9]*m[14]+m[4]*m[10]*m[13]+m[8]*m[5]*m[14]-m[8]*m[6]*m[13]-m[12]*m[5]*m[10]+m[12]*m[6]*m[9];
		inv[1] = -m[1]*m[10]*m[15]+m[1]*m[11]*m[14]+m[9]*m[2]*m[15]-m[9]*m[3]*m[14]-m[13]*m[2]*m[11]+m[13]*m[3]*m[10];
		inv[5] = m[0]*m[10]*m[15]-m[0]*m[11]*m[14]-m[8]*m[2]*m[15]+m[8]*m[3]*m[14]+m[12]*m[2]*m[11]-m[12]*m[3]*m[10];
		inv[9] = -m[0]*m[9]*m[15]+m[0]*m[11]*m[13]+m[8]*m[1]*m[15]-m[8]*m[3]*m[13]-m[12]*m[1]*m[11]+m[12]*m[3]*m[9];
		inv[13] = m[0]*m[9]*m[14]-m[0]*m[10]*m[13]-m[8]*m[1]*m[14]+m[8]*m[2]*m[13]+m[12]*m[1]*m[10]-m[12]*m[2]*m[9];
		inv[2] = m[1]*m[6]*m[15]-m[1]*m[7]*m[14]-m[5]*m[2]*m[15]+m[5]*m[3]*m[14]+m[13]*m[2]*m[7]-m[13]*m[3]*m[6];
		inv[6] = -m[0]*m[6]*m[15]+m[0]*m[7]*m[14]+m[4]*m[2]*m[15]-m[4]*m[3]*m[14]-m[12]*m[2]*m[7]+m[12]*m[3]*m[6];
		inv[10] = m[0]*m[5]*m[15]-m[0]*m[7]*m[13]-m[4]*m[1]*m[15]+m[4]*m[3]*m[13]+m[12]*m[1]*m[7]-m[12]*m[3]*m[5];
		inv[14] = -m[0]*m[5]*m[14]+m[0]*m[6]*m[13]+m[4]*m[1]*m[14]-m[4]*m[2]*m[13]-m[12]*m[1]*m[6]+m[12]*m[2]*m[5];
		inv[3] = -m[1]*m[6]*m[11]+m[1]*m[7]*m[10]+m[5]*m[2]*m[11]-m[5]*m[3]*m[10]-m[9]*m[2]*m[7]+m[9]*m[3]*m[6];
		inv[7] = m[0]*m[6]*m[11]-m[0]*m[7]*m[10]-m[4]*m[2]*m[11]+m[4]*m[3]*m[10]+m[8]*m[2]*m[7]-m[8]*m[3]*m[6];
		inv[11] = -m[0]*m[5]*m[11]+m[0]*m[7]*m[9]+m[4]*m[1]*m[11]-m[4]*m[3]*m[9]-m[8]*m[1]*m[7]+m[8]*m[3]*m[5];
		inv[15] = m[0]*m[5]*m[10]-m[0]*m[6]*m[9]-m[4]*m[1]*m[10]+m[4]*m[2]*m[9]+m[8]*m[1]*m[6]-m[8]*m[2]*m[5];
		const det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
		for (let i = 0; i < 16; i++) inv[i] /= det;
		return inv;
	},
	transform(m, v) {
		const x = v[0], y = v[1], z = v[2];
		return [m[0]*x + m[4]*y + m[8]*z + m[12], m[1]*x + m[5]*y + m[9]*z + m[13], m[2]*x + m[6]*y + m[10]*z + m[14], m[3]*x + m[7]*y + m[11]*z + m[15]];
	}
};

