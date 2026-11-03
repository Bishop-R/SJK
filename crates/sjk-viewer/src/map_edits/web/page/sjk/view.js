// ---- the view: the fly camera, a click on the map, and a frame
//
// The frame is VisionJK's (js/frame.js) with only the map: the map with its shader scripts once they are read
// (the plain view of lightmaps and images before), the models of misc_model_static, SJK's placed models and
// shown faces, what is see-through, the flares.

const keys = new Set();
cam.fly = true;

// W A S D (or the arrows) along the view, Q and E (or C and Space) down and up, Shift three times as fast; the
// wheel sets the speed
function moveCamera(dt) {
	const y = cam.yaw * Math.PI / 180, d = camDir(), v = cam.flySpeed * (keys.has('shift') ? 3 : 1) * dt;
	const fwd = (keys.has('w') || keys.has('arrowup') ? 1 : 0) - (keys.has('s') || keys.has('arrowdown') ? 1 : 0);
	const right = (keys.has('d') || keys.has('arrowright') ? 1 : 0) - (keys.has('a') || keys.has('arrowleft') ? 1 : 0);
	const up = (keys.has('e') || keys.has(' ') ? 1 : 0) - (keys.has('q') || keys.has('c') ? 1 : 0);
	cam.pos[0] += (d[0] * fwd + Math.sin(y) * right) * v;
	cam.pos[1] += (d[1] * fwd - Math.cos(y) * right) * v;
	cam.pos[2] += (d[2] * fwd + up) * v;
}

// the plain view (js/frame.js drawWorldPlain, as in the game: its light, no haze) until the scripts are read
function drawWorldPlain() {
	gl.useProgram(mapProg.p);
	gl.uniformMatrix4fv(mapProg.u.uMvp, false, viewProj);
	gl.uniform3fv(mapProg.u.uEye, eye);
	gl.uniform1f(mapProg.u.uFogFar, 4000);
	gl.uniform1i(mapProg.u.uTex, 0);
	gl.uniform1i(mapProg.u.uLm, 1);
	gl.uniform1i(mapProg.u.uView, 3);
	gl.disable(gl.CULL_FACE);
	for (const b of world.batches) {
		const drawn = remapOf(b.shaderName);
		let t = world.textures.get(drawn) || world.textures.get(b.shaderName);
		if (!t || t.failed) t = b.editorTex || (b.editorTex = editorImage(b.shaderName)) || t;
		if (!t) continue;
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, t.tex);
		gl.activeTexture(gl.TEXTURE1);
		gl.bindTexture(gl.TEXTURE_2D, b.lm || grayTex);
		gl.uniform1i(mapProg.u.uUseLm, b.lm ? 1 : 0);
		gl.uniform1i(mapProg.u.uAlphaTest, t.alpha ? 1 : 0);
		gl.bindVertexArray(b.vao);
		gl.drawElements(gl.TRIANGLES, b.count, gl.UNSIGNED_INT, 0);
	}
	gl.activeTexture(gl.TEXTURE0);
}

// a mesh of js/gl.js at a point, in one colour
function sjkMark(mesh, pos, yaw, scale, color) {
	const c = Math.cos(yaw), s = Math.sin(yaw);
	gl.useProgram(flatProg.p);
	gl.uniformMatrix4fv(flatProg.u.uMvp, false, viewProj);
	gl.uniformMatrix4fv(flatProg.u.uModel, false, new Float32Array([c * scale, s * scale, 0, 0, -s * scale, c * scale, 0, 0, 0, 0, scale, 0, pos[0], pos[1], pos[2], 1]));
	gl.uniform4fv(flatProg.u.uColor, color);
	gl.bindVertexArray(mesh.vao);
	gl.drawElements(gl.TRIANGLES, mesh.count, gl.UNSIGNED_SHORT, 0);
}

// the point the selection was made at, and the placed model Place moves
function sjkDrawMarks(now) {
	const s = sjk.state;
	if (!s) return;
	if (s.selection) sjkMark(cubeMesh, s.selection.hit, now / 700, 0.5, [0.85, 0.7, 0.35, 1]);
	const current = s.current && s.placed.find(p => p.id === s.current);
	if (current) sjkMark(ringMesh, current.origin, now / 900, 0.5, [0.85, 0.7, 0.35, 1]);
	gl.bindVertexArray(null);
}

const frameErrors = new Set();
let sjkBefore = performance.now();
function frame(now) {
	try {
		drawFrame(now);
	} catch (e) {
		const text = String(e && e.stack || e);
		if (!frameErrors.has(text)) { frameErrors.add(text); console.error('a frame failed: ' + text); }
		gl.bindVertexArray(null);
		gl.depthMask(true); gl.depthFunc(gl.LESS); gl.depthRange(0, 1);
		gl.colorMask(true, true, true, true);
		gl.disable(gl.BLEND); gl.disable(gl.CULL_FACE); gl.disable(gl.POLYGON_OFFSET_FILL);
	}
	requestAnimationFrame(frame);
}
function drawFrame(now) {
	const dt = Math.min(0.1, (now - sjkBefore) / 1000);
	sjkBefore = lastFrame = now;
	if (!document.hidden) moveCamera(dt);
	if (canvas.width !== Math.max(1, Math.round(canvas.clientWidth * canvasScale()))) resize();
	updateCamera();
	frameNow = now;
	if (world) {
		if (settleSky(world)) prepareScripted(world);
		updateGridLod(now, gameLook() && !!world.scripted);
		// the edits of the state that came before the map was read
		if (sjk.world !== world) sjkApply();
	}
	gl.viewport(0, 0, canvas.width, canvas.height);
	gl.clearColor(0.03, 0.035, 0.05, 1);
	gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
	gl.enable(gl.DEPTH_TEST);
	if (!world) return;
	const scripted = gameLook() && world.scripted;
	lateModels.length = 0;
	deferLate = !!scripted;
	if (scripted) {
		skyPortalOn = drawSkyPortal(now);
		drawWorldScripted(now, false);
	} else {
		drawWorldPlain();
	}
	drawWorldEnts(now);
	sjkDrawEdits();
	sjkDrawMarks(now);
	if (scripted) {
		drawWorldScripted(now, true);
		drawFlares(now);
	}
	deferLate = false;
}

// ---- mouse and keys on the map

const typing = () => { const a = document.activeElement; return !!a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA' || a.isContentEditable); };
let drag = null;
canvas.addEventListener('contextmenu', e => e.preventDefault());
canvas.addEventListener('pointerdown', e => {
	if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
	drag = { x: e.clientX, y: e.clientY, moved: false, button: e.button };
	canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener('pointermove', e => {
	if (!drag) return;
	const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
	if (!drag.moved && Math.hypot(dx, dy) < 4) return;
	drag.moved = true;
	drag.x = e.clientX;
	drag.y = e.clientY;
	// any button held looks around
	cam.yaw -= dx * 0.25;
	cam.pitch = Math.max(-89, Math.min(89, cam.pitch + dy * 0.25));
});
canvas.addEventListener('pointerup', e => {
	const was = drag;
	drag = null;
	if (!was || was.moved || was.button !== 0 || !world || !viewProj) return;
	const r = canvas.getBoundingClientRect(), ray = sjkRay(e.clientX - r.left, e.clientY - r.top);
	// on Place with a model chosen a click places it there; else it selects
	if (sjkTab === 'place' && sjk.chosenModel) sjkEdit(Object.assign({ op: 'place', model: sjk.chosenModel }, ray));
	else {
		sjkEdit(Object.assign({ op: 'select' }, ray));
		if (sjkTab === 'edits') sjkShowTab('selected');
	}
});
canvas.addEventListener('wheel', e => {
	e.preventDefault();
	cam.flySpeed = Math.max(50, Math.min(4000, cam.flySpeed * (e.deltaY < 0 ? 1.2 : 1 / 1.2)));
	toast('Speed ' + Math.round(cam.flySpeed));
}, { passive: false });
window.addEventListener('keydown', e => {
	if (typing()) return;
	const k = e.key.toLowerCase();
	if ((e.ctrlKey || e.metaKey) && k === 'z') { e.preventDefault(); sjkEdit({ op: e.shiftKey ? 'redo' : 'undo' }); return; }
	if ((e.ctrlKey || e.metaKey) && k === 'y') { e.preventDefault(); sjkEdit({ op: 'redo' }); return; }
	if (e.ctrlKey || e.metaKey || e.altKey) return;
	if (k === 'delete' && sjk.chosenEdit !== null && sjkTab === 'edits') { sjkEdit({ op: 'delete', id: sjk.chosenEdit }); return; }
	if (k === 'h' && sjk.state && sjk.state.selection) { sjkEdit({ op: 'hide' }); return; }
	keys.add(k);
	if (k === ' ' || k.startsWith('arrow')) e.preventDefault();
});
window.addEventListener('keyup', e => { keys.delete(e.key.toLowerCase()); if (e.key === 'Shift') keys.delete('shift'); });
window.addEventListener('blur', () => keys.clear());
window.addEventListener('resize', resize);

// ---- the start

if (!gl) {
	$('loading').textContent = 'This browser has no WebGL 2.';
} else {
	resize();
	// the map as the game draws it: its shader scripts (api/shaders)
	loadShaderScripts();
	sjkShowTab('selected');
	sjkPoll();
	requestAnimationFrame(frame);
}
