// ---- the panel: the in-game editor's pages (Selected, Place, Edits) and its foot (scope, answer, Publish)
//
// Every button asks the game for the command the in-game panel's button runs (sjkEdit), so both stay one editor.

const SJK_ROWS = 200, SJK_TILES = 120;
let sjkTab = 'selected';

function sjkShowTab(tab) {
	sjkTab = tab;
	for (const b of document.querySelectorAll('#tabs button')) b.classList.toggle('on', b.dataset.tab === tab);
	for (const t of ['selected', 'place', 'edits']) $('tab-' + t).hidden = t !== tab;
	if (tab === 'selected') sjkRenderShaders();
	if (tab === 'place') sjkRenderModels();
}

// what changes with every state: the header, the selection, the edits, the foot
function sjkRender() {
	const s = sjk.state;
	$('mapName').textContent = s ? s.map + (s.checksum !== null && s.checksum !== undefined ? '' : ' (reading its checksum...)') : '';
	const problem = sjk.error || (s && !s.editor ? 'The map editor is off in the game: mapedit turns it on' : '') ||
		(s && !s.enabled ? 'SJK map edits are off in the game (cg_mapEdits 0): edits are made but not drawn' : '');
	$('problem').textContent = problem;
	$('problem').hidden = !problem;
	$('busy').hidden = !sjk.busy;
	if (!s) return;
	// Selected
	const sel = s.selection;
	$('selSummary').textContent = sel ? sel.summary : 'Click the map to select a prop, a surface, a placed model or a hidden face';
	$('selKind').textContent = sel ? { model: 'Map prop', placed: 'Placed model', surface: 'Surface', face: 'Hidden face', other: 'Not editable' }[sel.kind] : '';
	const editable = sel && sel.kind !== 'other';
	$('btnHide').disabled = !editable;
	$('btnHide').textContent = sel && sel.kind === 'placed' ? 'Remove' : 'Hide';
	$('btnHideGroup').disabled = !sel || sel.kind !== 'surface';
	$('btnCopy').disabled = !sel || (sel.kind !== 'model' && sel.kind !== 'placed');
	$('btnRemap').disabled = !editable || !sjkRemapTarget();
	$('btnUnremap').disabled = !editable;
	// Place
	const current = s.edits.find(e => e.id === s.current);
	$('currentName').textContent = current ? current.label : 'none: place one, or choose one in Edits';
	for (const b of document.querySelectorAll('#moves button')) b.disabled = !current;
	// Edits
	const list = $('editList'), key = JSON.stringify([s.edits.map(e => [e.id, e.label, e.active]), s.current, sjk.chosenEdit]);
	if (list.dataset.key !== key) {
		list.dataset.key = key;
		list.innerHTML = s.edits.length ? s.edits.map(e => '<div class="row' + (e.active ? '' : ' dim') + (e.id === sjk.chosenEdit ? ' sel' : '') + '" data-id="' + e.id + '" title="' +
			escapeHtml(e.describe) + '">' + e.id + ': ' + escapeHtml(e.label) + (e.id === s.current ? ' <b class="gold">moved by Place</b>' : '') + '</div>').join('')
			: '<div class="hint">No edits on this map yet.</div>';
	}
	$('btnDelete').disabled = !s.edits.some(e => e.id === sjk.chosenEdit);
	$('btnUndo').disabled = !s.undo;
	$('btnRedo').disabled = !s.redo;
	// the foot
	for (const b of document.querySelectorAll('#scope button')) b.classList.toggle('on', b.dataset.scope === s.scope);
	$('scopeServer').textContent = s.server ? 'This server (' + s.server + ')' : 'This server';
	const answer = sjk.answer || null;
	const text = answer ? answer.text : s.hub ? 'Hub: ' + s.hub : '';
	$('answer').textContent = text;
	$('answer').classList.toggle('err', !!(answer ? answer.error : s.hub));
	$('btnPublish').classList.toggle('gold', !!s.unpublished);
	$('btnPublish').disabled = !s.edits.length && !s.unpublished;
	$('pubState').textContent = s.unpublished ? 'Changed here, not published' : s.revision ? 'Hub revision ' + s.revision : '';
}

// the lists from the game: shown on their pages
function sjkRenderLists() {
	sjkRenderShaders();
	sjkRenderModels();
}

const sjkWords = text => text.toLowerCase().split(/\s+/).filter(Boolean);
const sjkMatches = (name, words) => words.every(w => name.toLowerCase().includes(w));

function sjkRenderModels() {
	const box = $('modelList');
	if (!sjk.lists) { box.innerHTML = '<div class="hint">Reading the models...</div>'; return; }
	const words = sjkWords($('modelFind').value), all = sjk.lists.models.filter(m => sjkMatches(m, words)), shown = all.slice(0, SJK_ROWS);
	box.innerHTML = shown.map(m => '<div class="row' + (m === sjk.chosenModel ? ' sel' : '') + '" data-model="' + escapeHtml(m) + '" title="' + escapeHtml(m) + '">' +
		escapeHtml(m.replace(/^models\//, '')) + '</div>').join('') + (all.length > shown.length ? '<div class="hint">' + (all.length - shown.length) + ' more: search to narrow</div>' : '') ||
		'<div class="hint">No model has that in its name.</div>';
	$('modelCount').textContent = all.length + ' models';
}

// what a remap draws: the shader chosen in the list, or the one typed
const sjkRemapTarget = () => ($('shaderFind').value.trim() && !sjk.lists?.shaders.includes(sjk.chosenShader) ? $('shaderFind').value.trim() : sjk.chosenShader) || '';

function sjkRenderShaders() {
	const box = $('shaderList');
	if (!sjk.lists) { box.innerHTML = '<div class="hint">Reading the shaders...</div>'; return; }
	const words = sjkWords($('shaderFind').value), all = sjk.lists.shaders.filter(m => sjkMatches(m, words)), shown = all.slice(0, SJK_TILES);
	box.innerHTML = shown.map(n => '<div class="tile' + (n === sjk.chosenShader ? ' sel' : '') + '" data-name="' + escapeHtml(n) + '" title="' + escapeHtml(n) + '"><i class="thumb" data-thumb="' +
		escapeHtml(n) + '"></i><span>' + escapeHtml(n.slice(n.lastIndexOf('/') + 1)) + '</span></div>').join('') ||
		'<div class="hint">No shader has that in its name; Remap tries the name as typed.</div>';
	$('shaderCount').textContent = all.length + ' shaders' + (all.length > shown.length ? ', the first ' + shown.length + ' shown' : '');
	thumbWatch(box);
}

// the arrows of Place: 8 units along the world axis nearest the camera's right and ahead, so steps add up
// straight (as the in-game panel moves); Shift: 1 unit, 1 degree, a tenth of the growth
function sjkNudge(right, away) {
	const y = cam.yaw * Math.PI / 180, r = [Math.sin(y), -Math.cos(y)], f = [Math.cos(y), Math.sin(y)];
	const axis = v => Math.abs(v[0]) >= Math.abs(v[1]) ? [Math.sign(v[0]), 0] : [0, Math.sign(v[1])];
	const a = axis(r), b = axis(f);
	return [a[0] * right + b[0] * away, a[1] * right + b[1] * away];
}
function sjkMove(e, kind) {
	const fine = e.shiftKey, step = fine ? 1 : 8;
	const moves = {
		up: [0, 0, step], down: [0, 0, -step],
		left: [...sjkNudge(-step, 0), 0], right: [...sjkNudge(step, 0), 0],
		away: [...sjkNudge(0, step), 0], closer: [...sjkNudge(0, -step), 0]
	};
	if (kind === 'turnl' || kind === 'turnr') return sjkEdit({ op: 'turn', degrees: (fine ? 1 : 15) * (kind === 'turnl' ? 1 : -1) });
	if (kind === 'bigger' || kind === 'smaller') { const g = fine ? 1.01 : 1.1; return sjkEdit({ op: 'scale', factor: kind === 'bigger' ? g : 1 / g }); }
	if (kind === 'remove') return sjk.state && sjk.state.current ? sjkEdit({ op: 'delete', id: sjk.state.current }) : null;
	return sjkEdit({ op: 'move', offset: moves[kind] });
}

function sjkPublish() {
	const s = sjk.state;
	if (!s) return;
	if (!confirm('Send this map\'s ' + s.edits.length + ' edits to the SJK hub for every SJK player? It replaces the hub\'s list for ' + s.map + '.')) return;
	sjkEdit({ op: 'publish' });
}

// ---- the panel's controls

for (const b of document.querySelectorAll('#tabs button')) b.addEventListener('click', () => sjkShowTab(b.dataset.tab));
$('btnHide').addEventListener('click', () => sjkEdit({ op: 'hide' }));
$('btnHideGroup').addEventListener('click', () => sjkEdit({ op: 'hide', group: true }));
$('btnCopy').addEventListener('click', () => {
	const sel = sjk.state && sjk.state.selection;
	if (!sel || !sel.target.model) return;
	sjk.chosenModel = sel.target.model;
	$('modelFind').value = '';
	sjkShowTab('place');
	toast('Click the map where the copy goes');
});
$('btnRemap').addEventListener('click', () => { const to = sjkRemapTarget(); if (to) sjkEdit({ op: 'remap', shader: to }); });
$('btnUnremap').addEventListener('click', () => sjkEdit({ op: 'remap', shader: '-' }));
let sjkTyped = 0;
$('shaderFind').addEventListener('input', () => { clearTimeout(sjkTyped); sjkTyped = setTimeout(() => { sjkRenderShaders(); sjkRender(); }, 150); });
$('shaderList').addEventListener('click', e => {
	const tile = e.target.closest('.tile');
	if (!tile) return;
	// a second click on the chosen one remaps, as in the game
	if (sjk.chosenShader === tile.dataset.name && sjk.state && sjk.state.selection) { sjkEdit({ op: 'remap', shader: tile.dataset.name }); return; }
	sjk.chosenShader = tile.dataset.name;
	for (const t of $('shaderList').querySelectorAll('.tile')) t.classList.toggle('sel', t === tile);
	sjkRender();
});
$('modelFind').addEventListener('input', () => { clearTimeout(sjkTyped); sjkTyped = setTimeout(sjkRenderModels, 150); });
$('modelList').addEventListener('click', e => {
	const row = e.target.closest('.row');
	if (!row) return;
	sjk.chosenModel = row.dataset.model;
	for (const r of $('modelList').querySelectorAll('.row')) r.classList.toggle('sel', r === row);
});
for (const b of document.querySelectorAll('#moves button')) b.addEventListener('click', e => sjkMove(e, b.dataset.move));
$('editList').addEventListener('click', e => {
	const row = e.target.closest('.row');
	if (!row) return;
	const id = +row.dataset.id, edit = sjk.state.edits.find(x => x.id === id);
	sjk.chosenEdit = id;
	sjkRender();
	// a placed model: the one Place moves
	if (edit && edit.edit.kind === 'place') sjkEdit({ op: 'current', id });
});
$('editList').addEventListener('dblclick', e => {
	const row = e.target.closest('.row');
	const edit = row && sjk.state.edits.find(x => x.id === +row.dataset.id);
	// a double click flies to a placed model
	if (edit && edit.edit.kind === 'place') {
		const o = edit.edit.origin, d = camDir();
		cam.fly = true;
		cam.pos = [o[0] - d[0] * 160, o[1] - d[1] * 160, o[2] - d[2] * 160 + 16];
	}
});
$('btnDelete').addEventListener('click', () => { if (sjk.chosenEdit !== null) sjkEdit({ op: 'delete', id: sjk.chosenEdit }); });
$('btnUndo').addEventListener('click', () => sjkEdit({ op: 'undo' }));
$('btnRedo').addEventListener('click', () => sjkEdit({ op: 'redo' }));
for (const b of document.querySelectorAll('#scope button')) b.addEventListener('click', () => sjkEdit({ op: 'scope', scope: b.dataset.scope }));
$('btnPublish').addEventListener('click', sjkPublish);
$('btnMyView').addEventListener('click', () => { if (sjk.state && sjk.state.view) sjkGoTo(sjk.state.view); });
