// From VisionJK's web admin page (codemp/game/webadmin/page/js/camera.js at 50d7c18b, GPL-2.0, by Blackwolf;
// the panel after JKAdmin's by Lixsl), for SJK's browser map editor. SJK: unchanged.
/* ------------------------------------------------------------------ */
/* camera (RTS style, Z up like the game)                              */
/* ------------------------------------------------------------------ */

// Two ways to look at the map. Around a point on it, from above (target, dist), as JKAdmin has it. Or flying:
// the camera is at pos and looks along yaw and pitch, which is how the inside of a map is looked at. A pitch
// above zero looks down in both.
const cam = { target: [0, 0, 0], yaw: 45, pitch: 55, dist: 1500, follow: -1, fly: false, pos: [0, 0, 0], flySpeed: 600 };

function camDir() {
	const y = cam.yaw * Math.PI / 180, p = cam.pitch * Math.PI / 180;
	return [Math.cos(y) * Math.cos(p), Math.sin(y) * Math.cos(p), -Math.sin(p)];
}

// into flying from where the camera is, and out of it to the point it looks at
function setFly(on) {
	if (on === cam.fly) return;
	if (on) {
		cam.pos = eye.slice();
		cam.follow = -1;
	} else {
		cam.pitch = Math.min(89, Math.max(10, cam.pitch));
		const d = camDir();
		cam.target = cam.pos.map((v, i) => v + d[i] * cam.dist);
	}
	cam.fly = on;
	$('flyBtn').classList.toggle('armedPlace', on);
}
let viewProj = null, invViewProj = null, eye = [0, 0, 0];

// Up to nine views of a map, kept by the browser: Shift and a digit stores where the camera is, the digit alone
// goes back there.
function camBookmark(n, store) {
	const key = 'views:' + loadedMap;
	let all = {};
	try { all = JSON.parse(localStorage.getItem(key) || '{}'); } catch (e) { /* none then */ }
	if (store) {
		all[n] = cam.fly ? { fly: true, pos: cam.pos, yaw: cam.yaw, pitch: cam.pitch } : { fly: false, target: cam.target, dist: cam.dist, yaw: cam.yaw, pitch: cam.pitch };
		try { localStorage.setItem(key, JSON.stringify(all)); toast('View ' + n + ' stored'); } catch (e) { toast('The browser does not keep it', false); }
		return;
	}
	const v = all[n];
	if (!v) { toast('No view under ' + n + ', Shift+' + n + ' stores one', false); return; }
	cam.follow = -1;
	setFly(!!v.fly);
	if (v.fly) cam.pos = v.pos.slice(); else { cam.target = v.target.slice(); cam.dist = v.dist; }
	cam.yaw = v.yaw;
	cam.pitch = v.pitch;
}

function resetCamera() {
	if (!world) return;
	setFly(false);
	const b = world.bounds;
	cam.target = [(b.mins[0] + b.maxs[0]) / 2, (b.mins[1] + b.maxs[1]) / 2, (b.mins[2] + b.maxs[2]) / 2];
	cam.dist = Math.max(b.maxs[0] - b.mins[0], b.maxs[1] - b.mins[1]) * 0.75;
}

// the camera looks at the middle of a box of the map, from no farther than before
function boxLookAt(b) {
	const mid = b.mins.map((v, i) => (v + b.maxs[i]) / 2);
	cam.follow = -1;
	cam.target = mid;
	cam.dist = Math.min(cam.dist, 900);
	if (cam.fly) { const d = camDir(); cam.pos = mid.map((v, i) => v - d[i] * 400); }
}

// The projection of the game, for every camera of the page: R_SetupProjection with what CG_CalcFov hands it.
// fovX is the angle across (cg_fov, 80) whatever the shape of the picture (cg_fovAspectAdjust is 0), the angle
// up and down follows from that shape, the near plane is r_znear and the far plane the one of SetFarClip.
// The far plane is worked out for where the eye is: eye is set before this is called.
const ZNEAR = 4;
const gameFovY = (w, h, fovX = 80) => 2 * Math.atan2(h, w / Math.tan(fovX * Math.PI / 360));
function gameProjection(w, h, fovX = 80, far = farClip()) {
	return M4.perspective(gameFovY(w, h, fovX), w / h, ZNEAR, far);
}

// The eye is where an eye of the game can be: the client the page is compared with, the fly camera, the eyes of
// a player. Around the map from above it is not, and what the game leaves out for being far away is left in
// there, as the fog is: it would take the map away that this view is there to show.
const camInMap = () => !!refCam || cam.fly || view.id >= 0;

// The far plane (SetFarClip in tr_main.cpp): as far as the farthest corner of the box around what the view draws
// of the map, at least 2048, and at most the distanceCull of the worldspawn times 1.732. The engine leaves no
// surface out for being far away, the map is cut where this plane is. Its box is the one around the leafs it
// draws; the page draws all of the map and takes the box around all of it. That is never nearer than the plane
// of the engine, and the same plane wherever distanceCull cuts the map.
function farClip(box = world && world.bounds, at = eye, cut = camInMap()) {
	if (!box) return 2048;
	let far = 0;
	for (let i = 0; i < 8; i++) {
		const x = (i & 1 ? box.mins : box.maxs)[0] - at[0], y = (i & 2 ? box.mins : box.maxs)[1] - at[1], z = (i & 4 ? box.mins : box.maxs)[2] - at[2];
		far = Math.max(far, x * x + y * y + z * z);
	}
	far = Math.sqrt(far);
	const most = mapEnts().distanceCull * 1.732;
	// Com_Clamp( 2048, most, far ), in its order
	return far < 2048 ? 2048 : cut && far > most ? most : far;
}

// A view from another place: the matrix of a view (projection times view) for an eye that is by d further on.
function viewMoved(vp, d, out) {
	out.set(vp);
	for (let r = 0; r < 4; r++) out[12 + r] = vp[12 + r] - vp[r] * d[0] - vp[4 + r] * d[1] - vp[8 + r] * d[2];
	return out;
}

// The same view with another far plane. Only the row of a projection that makes the depth has the far plane in
// it (R_SetupProjection), and the row below it is the depth before the far plane comes into it.
function viewWithFar(vp, far, out) {
	const a = (far + ZNEAR) / (ZNEAR - far), b = 2 * far * ZNEAR / (ZNEAR - far);
	out.set(vp);
	out[2] = -a * vp[3]; out[6] = -a * vp[7]; out[10] = -a * vp[11]; out[14] = b - a * vp[15];
	return out;
}

// The camera of the comparison with the game: the eye and the angles of a client, as /api/state?view= tells
// them (origin, pitch yaw roll, viewheight, pm_type, time of the level). null: the camera of the page again.
let refCam = null;
function refView(v) {
	refCam = v ? { eye: [v[0], v[1], v[2] + v[6]], pitch: v[3], yaw: v[4], time: v[8] } : null;
}
// The comparison with a time: the time of the game stands for the page too (what animates is at that time, and
// nothing runs on between two polls). Without a time (0) it is only the camera of the game, and the page goes on
// as it does for anybody who looks at it.
const refStands = () => !!refCam && !!refCam.time;

function updateCamera() {
	let at;
	if (refCam) {
		// the way a client looks: AngleVectors, a pitch above zero looks down
		const p = refCam.pitch * Math.PI / 180, y = refCam.yaw * Math.PI / 180, d = [Math.cos(p) * Math.cos(y), Math.cos(p) * Math.sin(y), -Math.sin(p)];
		eye = refCam.eye.slice();
		at = eye.map((v, i) => v + d[i]);
	} else {
		// following a player is done around him
		if (cam.fly && cam.follow >= 0) setFly(false);
		if (cam.fly) {
			const d = camDir();
			eye = cam.pos.slice();
			at = eye.map((v, i) => v + d[i]);
		} else {
			if (cam.follow >= 0) {
				const p = players.get(cam.follow);
				if (p) cam.target = p.draw.slice();
			}
			const y = cam.yaw * Math.PI / 180, pt = cam.pitch * Math.PI / 180;
			eye = [cam.target[0] - Math.cos(y) * Math.cos(pt) * cam.dist, cam.target[1] - Math.sin(y) * Math.cos(pt) * cam.dist, cam.target[2] + Math.sin(pt) * cam.dist];
			at = cam.target;
		}
	}
	viewProj = M4.mul(gameProjection(canvas.width, canvas.height), M4.lookAt(eye, at, [0, 0, 1]));
	invViewProj = M4.invert(viewProj);
}

function screenRay(x, y) {
	const nx = x / canvas.clientWidth * 2 - 1, ny = 1 - y / canvas.clientHeight * 2;
	const a = M4.transform(invViewProj, [nx, ny, -1]), b = M4.transform(invViewProj, [nx, ny, 1]);
	const p0 = [a[0] / a[3], a[1] / a[3], a[2] / a[3]], p1 = [b[0] / b[3], b[1] / b[3], b[2] / b[3]];
	const d = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]], l = Math.hypot(...d);
	return { origin: p0, dir: d.map(v => v / l) };
}

function project(p) {
	const c = M4.transform(viewProj, p);
	if (c[3] <= 0) return null;
	return [(c[0] / c[3] * 0.5 + 0.5) * canvas.clientWidth, (0.5 - c[1] / c[3] * 0.5) * canvas.clientHeight];
}

// ---- the camera behind a player (CG_OffsetThirdPersonView in cg_view.c, with the cvars at their defaults)
//
// The game looks at a point 16 above the eye of the player (cg_thirdPersonVertOffset) from 80 behind it along his
// view (cg_thirdPersonRange). Both follow him with a delay (cg_thirdPersonTargetDamp 0.5, cg_thirdPersonCameraDamp
// 0.3, stiffer the faster he turns), and both stop at what is in the way: a box of 4 to every side is traced from
// his eye to the point and from the point to the camera (MASK_CAMERACLIP). Not here: the camera of a vehicle and of
// one a rancor holds, and movers in the way (the trace of the page knows the map alone).
const CHASE = { range: 80, vertOffset: 16, targetDamp: 0.5, cameraDamp: 0.3, interval: 50, mins: [-4, -4, -4], maxs: [4, 4, 4], mask: 0x1 | 0x1000 | 0x10 };
const chase = { lastFrame: 0, lastYaw: 0, of: -1, curTarget: [0, 0, 0], curLoc: [0, 0, 0] };

// origin: where the player is, angles: the way he looks (pitch, yaw), time: the time of the frame in msec, or -1
// for a camera that has come to rest (the comparison with the game, where the time stands). deadYaw: where one who
// is dead is looked at from (STAT_DEAD_YAW), undefined for one who lives. Returns the eye and the way it looks.
function chaseCamera(id, origin, viewheight, pitch, yaw, time, deadYaw) {
	const C = CHASE, c = chase;
	if (deadYaw !== undefined && deadYaw !== null) yaw = deadYaw;
	const reset = time < 0 || c.of !== id || c.lastFrame === 0 || c.lastFrame > time;
	// the pitch is capped at 89 for a camera that starts and at 80 for one that follows
	const cap = reset ? 89 : 80;
	pitch = Math.max(-cap, Math.min(cap, pitch));
	const pr = pitch * Math.PI / 180, yr = yaw * Math.PI / 180, fwd = [Math.cos(pr) * Math.cos(yr), Math.cos(pr) * Math.sin(yr), -Math.sin(pr)];
	// CG_CalcIdealThirdPersonViewTarget and ...Location
	const focus = [origin[0], origin[1], origin[2] + viewheight], idealTarget = [focus[0], focus[1], focus[2] + C.vertOffset];
	const idealLoc = idealTarget.map((v, i) => v - C.range * fwd[i]);
	const clip = (from, to) => traceBox(from, to, C.mins, C.maxs, C.mask);
	if (reset) {
		// CG_ResetThirdPersonViewDamp: everything at its place at once
		c.curTarget = clip(focus, idealTarget).pos.slice();
		c.curLoc = clip(c.curTarget, idealLoc).pos.slice();
		c.of = id;
	} else {
		let dyaw = Math.abs(yaw - c.lastYaw);
		if (dyaw > 180) dyaw = Math.abs(dyaw - 360);
		const dt = time - c.lastFrame, dtime = dt / C.interval;
		let stiff = dt > 0 ? dyaw / dt : 0;
		stiff = stiff < 1 ? 0 : stiff > 2.5 ? 0.75 : (stiff - 1) * 0.5;
		// CG_UpdateThirdPersonTargetDamp: what is left of the way to the point after this time
		let ratio = Math.pow(1 - C.targetDamp, dtime);
		c.curTarget = idealTarget.map((v, i) => v - ratio * (v - c.curTarget[i]));
		let tr = clip(focus, c.curTarget);
		if (tr.fraction < 1) c.curTarget = tr.pos.slice();
		// CG_UpdateThirdPersonCameraDamp: less of a delay the more he looks up or down, and the faster he turns
		const pf = Math.abs(pitch) / 115;
		let damp = (1 - C.cameraDamp) * pf * pf + C.cameraDamp;
		if (stiff > 0) damp += (1 - damp) * stiff;
		if (damp >= 1) c.curLoc = idealLoc.slice();
		else { ratio = Math.pow(1 - damp, dtime); c.curLoc = idealLoc.map((v, i) => v - ratio * (v - c.curLoc[i])); }
		tr = clip(c.curTarget, c.curLoc);
		if (tr.fraction < 1) c.curLoc = tr.pos.slice();
	}
	c.lastYaw = yaw;
	c.lastFrame = time < 0 ? 0 : time;
	// the camera looks from where it is to the point, or along the view of the player where the two are one
	let dir = [c.curTarget[0] - c.curLoc[0], c.curTarget[1] - c.curLoc[1], c.curTarget[2] - c.curLoc[2]];
	const dist = Math.hypot(dir[0], dir[1], dir[2]);
	if (!dist || dir[0] === 0 || dir[1] === 0) dir = fwd; else dir = dir.map(v => v / dist);
	return { eye: c.curLoc.slice(), dir };
}
