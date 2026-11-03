// ---- models: reading an MD3 and drawing it at a place
//
// Taken from VisionJK's web admin page (at 50d7c18b, GPL-2.0, by Blackwolf; the panel after JKAdmin's by Lixsl) (codemp/game/webadmin/page/js), parts of three of its files put
// together for SJK's map editor page, which draws no players or weapons: readStr and parseMd3 from picker.js,
// fetchGameFile, cached, mainTexture and skinProg from g2.js, and from weapons.js the matrices, modelVao and the
// static models (loadStaticModel without its Ghoul2 branch, staticNow, drawStatic). Otherwise unchanged.

function readStr(dv, o, len) {
	let s = '';
	for (let i = 0; i < len; i++) { const c = dv.getUint8(o + i); if (!c) break; s += String.fromCharCode(c); }
	return s;
}

function parseMd3(buf) {
	const dv = new DataView(buf), surfs = [];
	if (readStr(dv, 0, 4) !== 'IDP3') return null;
	let o = dv.getInt32(100, true);
	for (let s = 0, n = dv.getInt32(84, true); s < n; s++) {
		const name = readStr(dv, o + 4, 64), numShaders = dv.getInt32(o + 76, true), nv = dv.getInt32(o + 80, true), nt = dv.getInt32(o + 84, true);
		const ofsTri = dv.getInt32(o + 88, true), ofsSh = dv.getInt32(o + 92, true), ofsSt = dv.getInt32(o + 96, true), ofsXyz = dv.getInt32(o + 100, true);
		const pos = new Float32Array(nv * 3), norm = new Float32Array(nv * 3), uv = new Float32Array(nv * 2), idx = new Uint32Array(nt * 3);
		for (let v = 0; v < nv; v++) {
			const b = o + ofsXyz + v * 8;
			pos[v * 3] = dv.getInt16(b, true) / 64; pos[v * 3 + 1] = dv.getInt16(b + 2, true) / 64; pos[v * 3 + 2] = dv.getInt16(b + 4, true) / 64;
			const lat = dv.getUint8(b + 7) * Math.PI * 2 / 256, lng = dv.getUint8(b + 6) * Math.PI * 2 / 256;
			norm[v * 3] = Math.cos(lat) * Math.sin(lng); norm[v * 3 + 1] = Math.sin(lat) * Math.sin(lng); norm[v * 3 + 2] = Math.cos(lng);
			uv[v * 2] = dv.getFloat32(o + ofsSt + v * 8, true); uv[v * 2 + 1] = dv.getFloat32(o + ofsSt + v * 8 + 4, true);
		}
		for (let i = 0; i < nt * 3; i++) idx[i] = dv.getInt32(o + ofsTri + i * 4, true);
		surfs.push({ name, shader: numShaders ? readStr(dv, o + ofsSh, 64) : '', pos, norm, uv, idx });
		o += dv.getInt32(o + 104, true);
	}
	return surfs;
}

async function fetchGameFile(path, asText) {
	// the game takes a name with a / in front as the one without it, and some of its own classes write it so
	path = path.replace(/^[\/\\]+/, '');
	for (let attempt = 0; ; attempt++) {
		try {
			const res = await fetch('api/file?name=' + encodeURIComponent(path));
			if (!res.ok) throw Object.assign(new Error(path + ': HTTP ' + res.status), { final: true });
			return await (asText ? res.text() : res.arrayBuffer());
		} catch (e) {
			if (e.final || attempt >= 10) throw e;
			await new Promise(done => setTimeout(done, 3000));
		}
	}
}

function cached(map, key, make) {
	if (!map.has(key)) map.set(key, make().catch(e => { console.warn(e); return null; }));
	return map.get(key);
}


const mainTextures = new Map();
function mainTexture(name) {
	const base = name.replace(/\.(tga|jpg|png)$/i, '');
	return cached(mainTextures, base, async () => {
		for (const ext of ['.jpg', '.png', '.tga']) {
			try {
				const data = await fetchGameFile(base + ext);
				if (ext === '.tga') {
					const img = decodeTga(data);
					if (img) return makeTexture(img.w, img.h, img.data);
				} else {
					const bmp = await createImageBitmap(new Blob([data]));
					const t = gl.createTexture();
					gl.bindTexture(gl.TEXTURE_2D, t);
					gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE, bmp);
					// mip levels, filter and anisotropy as the engine has them (js/gl.js)
					finishTexture(bmp.width, bmp.height, {});
					return t;
				}
			} catch (e) { /* next extension */ }
		}
		return grayTex;
	}).then(t => t || grayTex);
}


const skinProg = compile(`#version 300 es
layout(location=0) in vec3 aPos; layout(location=1) in vec3 aNorm; layout(location=2) in vec2 aUv;
uniform mat4 uMvp; uniform mat4 uModel; out vec3 vN; out vec2 vUv;
void main() { vN = mat3(uModel) * aNorm; vUv = aUv; gl_Position = uMvp * uModel * vec4(aPos, 1.0); }`,
`#version 300 es
precision mediump float;
in vec3 vN; in vec2 vUv; uniform sampler2D uTex; uniform vec3 uTint; uniform float uTintAmount; out vec4 o;
void main() {
	vec4 t = texture(uTex, vUv);
	vec3 n = normalize(vN);
	float l = 0.5 + 0.6 * max(dot(n, normalize(vec3(0.4, 0.3, 0.85))), 0.0);
	o = vec4(mix(t.rgb * l, uTint, uTintAmount), 1.0);
}`);


// 3x4 row major to 4x4 column major
const to44 = m => new Float32Array([m[0], m[4], m[8], 0, m[1], m[5], m[9], 0, m[2], m[6], m[10], 0, m[3], m[7], m[11], 1]);

// where a Ghoul2 model stands in the world (it looks down -Y, the game's yaw 0 is +X)
function entityMatrix(x, y, z, yaw, scale) {
	const a = yaw + Math.PI / 2, c = Math.cos(a) * scale, s = Math.sin(a) * scale;
	return new Float32Array([c, s, 0, 0, -s, c, 0, 0, 0, 0, scale, 0, x, y, z, 1]);
}

// where an entity draws its model: the axes of the game for its angles (AnglesToAxis), scaled, at its origin
function anglesMatrix(pos, angles, scale) {
	const r = Math.PI / 180, sp = Math.sin(angles[0] * r), cp = Math.cos(angles[0] * r), sy = Math.sin(angles[1] * r), cy = Math.cos(angles[1] * r),
		sr = Math.sin(angles[2] * r), cr = Math.cos(angles[2] * r);
	return new Float32Array([
		cp * cy * scale, cp * sy * scale, -sp * scale, 0,
		(sr * sp * cy - cr * sy) * scale, (sr * sp * sy + cr * cy) * scale, sr * cp * scale, 0,
		(cr * sp * cy + sr * sy) * scale, (cr * sp * sy - sr * cy) * scale, cr * cp * scale, 0,
		pos[0], pos[1], pos[2], 1]);
}

// a mesh for the programs that draw models: positions, normals, texture coordinates and triangles
function modelVao(pos, norm, uv, idx) {
	const vao = gl.createVertexArray();
	gl.bindVertexArray(vao);
	[[0, pos, 3], [1, norm, 3], [2, uv, 2]].forEach(([loc, data, size]) => {
		const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b); gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
		gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, size, gl.FLOAT, false, 0, 0);
	});
	const ib = gl.createBuffer(); gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib); gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
	gl.bindVertexArray(null);
	return vao;
}


// a static model, an MD3 (VisionJK's loadStaticModel reads Ghoul2 hilts too; SJK places MD3s only)
const staticModels = new Map();
function loadStaticModel(path) {
	return cached(staticModels, path, async () => {
		const buf = await fetchGameFile(path);
		const surfs = parseMd3(buf);
		if (!surfs) return null;
		const meshes = [];
		for (const s of surfs) {
			const vao = modelVao(s.pos, s.norm, s.uv, s.idx);
			const shader = s.shader && s.shader !== '[nomaterial]' ? s.shader : '';
			meshes.push({ vao, count: s.idx.length, tex: shader ? await mainTexture(shader) : grayTex, shaderName: shader, mat: null });
		}
		return { meshes, tags: {} };
	});
}

// the model if it is loaded, else starts loading it and returns null
const staticReady = new Map();
function staticNow(path) {
	if (staticReady.has(path)) return staticReady.get(path);
	if (!staticModels.has(path)) loadStaticModel(path).then(m => staticReady.set(path, m));
	return null;
}


function drawStatic(model, world) {
	if (modelsScripted()) { drawModelScripted(model.meshes, world); gl.useProgram(flatProg.p); return; }
	gl.useProgram(skinProg.p);
	gl.uniformMatrix4fv(skinProg.u.uMvp, false, viewProj);
	gl.uniformMatrix4fv(skinProg.u.uModel, false, world);
	gl.uniform3fv(skinProg.u.uTint, [0, 0, 0]);
	gl.uniform1f(skinProg.u.uTintAmount, 0);
	gl.uniform1i(skinProg.u.uTex, 0);
	gl.activeTexture(gl.TEXTURE0);
	for (const mesh of model.meshes) {
		gl.bindTexture(gl.TEXTURE_2D, mesh.tex);
		gl.bindVertexArray(mesh.vao);
		gl.drawElements(gl.TRIANGLES, mesh.count, gl.UNSIGNED_INT, 0);
	}
	gl.useProgram(flatProg.p);
}

