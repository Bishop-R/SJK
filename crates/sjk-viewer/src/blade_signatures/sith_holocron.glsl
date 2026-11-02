// Sith Holocron: the blade is a holocron's crimson crystal stretched into a blade: a real
// three-sided crystal prism with a slow twist, its glass cut into triangular facets that glint
// as you turn it, coming to a point at the tip. A red heart beats deep inside it, and Sith
// knowledge streams up through the glass as thin lines of light, facet after facet waking as the
// stream passes. Every ten seconds the holocron opens: the crystal splits along a seam, the halves
// part on a white-hot core, and red smoke and lightning pour out of the crack all round before it
// seals again. A swing makes the facets flare and lightning leap off the glass; a clash throws out
// a triangular shockwave; on ignition the crystal grows from the hilt, facets waking as it rises.
// @part Holocron
// @part Lightning storm
// @arcs Sith lightning
// @wisps Red smoke
// @room 10
// @solid 1
// @palette {"corona.rim_cool":[0.40,0.0,0.02],"corona.rim_hot":[1.0,0.10,0.06],"corona.inner":[1.0,0.65,0.60],"core.white":[1.0,0.92,0.90],"core.fringe_cool":[0.85,0.03,0.04],"core.fringe_hot":[1.0,0.30,0.22],"light.color":[1.0,0.12,0.08],"trail":[1.0,0.12,0.08]}

const float HC_A = 0.45;       // the crystal's inner radius (its faces' distance from the axis)
float hc_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
// how far the holocron is open: 0 sealed .. 1 split wide
float hc_open(){
  float c = mod(uTime - 2.0, 10.0);
  return smoothstep(0.0, 0.9, c)*(1.0 - smoothstep(3.2, 4.2, c));
}
// the seam's half gap at height y (widest in the middle of the blade)
float hc_gap(float y){
  float u = clamp(y/uLength, 0.0, 1.0);
  return 0.30*hc_open()*pow(max(sin(3.1416*u), 0.0), 0.6);
}
float hc_beat(){ return 0.55 + 0.3*pow(0.5 + 0.5*sin(uTime*2.4), 6.0) + 0.25*pow(0.5 + 0.5*sin(uTime*2.4 - 0.7), 10.0); }
// the crystal's half width at y: a short neck at the hilt, drawn to a point at the tip
float hc_a(float y){
  float top = hc_top();
  return HC_A*1.45*(1.0 - smoothstep(top - 2.6, top + 0.2, y)*0.97)*smoothstep(-0.2, 0.5, y);
}
// a triangle lattice: x = distance to the nearest facet edge, yz = the facet's id
vec3 hc_tri(vec2 p){
  vec2 s = vec2(p.x + p.y*0.57735, p.y*1.1547);
  vec2 i = floor(s), f = fract(s);
  float up = step(f.x + f.y, 1.0);
  float e = up > 0.5 ? min(min(f.x, f.y), 1.0 - f.x - f.y) : min(min(1.0 - f.x, 1.0 - f.y), f.x + f.y - 1.0);
  return vec3(e*0.866, i + vec2(up*0.5, 0.0));
}
// the knowledge stream: height of the waking front
float hc_front(){ return mod(uTime*9.0, uLength + 16.0) - 4.0; }
// the crystal, drawn over Sol's glow and core as his solid blades are: a three-sided prism
// facing the camera (two faces seen, meeting on a ridge that turns with its slow twist), its
// glass cut into triangular facets, the heart beating red through it; it splits along the
// seam when the holocron opens and the white-hot core shows between the halves
vec4 sig_solid(vec4 o, vec2 b, float px){
  float L = uLength, t = uTime, top = hc_top();
  if (top < 0.3) return o;
  float y = b.y;
  float a = hc_a(y);
  float g = hc_gap(y);
  float side = b.x >= 0.0 ? 1.0 : -1.0;
  float x = abs(b.x) - g;
  if (x < -0.001 && g > 0.001) return o;            // the open seam: the core shows through
  float xs = side*x;
  float cov = fillD(abs(xs) - a, px)*step(0.1, y)*(1.0 - smoothstep(top - 0.05, top + 0.3, y));
  if (cov <= 0.0) return o;
  // the ridge where two faces meet wanders across with the twist
  float ridge = a*0.7*sin(0.35 + y*0.045 + t*0.25);
  float fid = xs > ridge ? 1.0 : 0.0;
  float u = (xs - ridge)/max(a, 0.01);
  vec3 tr = hc_tri(vec2(u*2.2 + fid*3.7 + 0.3*side, y*0.45));
  float id = h21(tr.yz + fid*13.0 + side*5.0);
  float awake = smoothstep(0.0, 0.25, uAge*2.2 - y/L - id*0.15);
  float beat = hc_beat();
  float fr = hc_front();
  float pass = exp(-pow((y - fr)/3.0, 2.0)) + 0.6*exp(-pow((y - fr + 14.0)/2.0, 2.0));
  float lk = y*2.6 - t*3.5 + id*0.5;
  float lines = halo(abs(fract(lk) - 0.5), 0.06)*step(0.55, h11(floor(lk) + fid*5.0 + side*3.0))*halo(abs(fract(u*3.0 + id) - 0.5), 0.12);
  // each facet glints in turn as the crystal turns
  float glint = pow(0.5 + 0.5*sin(t*1.3 + id*12.0 + fid*2.0), 12.0);
  float lit = clamp(0.12 + 0.9*id*id*beat + pass*(0.5 + 0.6*id) + 0.9*uSwing*step(0.5, id), 0.0, 2.0);
  vec3 face = mix(vec3(0.22, 0.0, 0.015), vec3(0.95, 0.10, 0.05), clamp(lit, 0.0, 1.0)*0.75) + vec3(1.0, 0.5, 0.4)*max(lit - 1.0, 0.0)*0.6;
  // the faces' shading: one face toward the light, the other away
  face *= fid > 0.5 ? 0.75 : 1.0;
  // the heart, deep inside at the axis, and the lines of knowledge
  face += vec3(1.0, 0.12, 0.06)*halo(b.x, a*0.55 + 0.05)*beat*0.9 + vec3(1.0, 0.55, 0.45)*halo(b.x, 0.12)*beat*0.35;
  face += vec3(1.0, 0.35, 0.25)*lines*0.8*(0.4 + pass);
  // cut lines between the facets, the ridge, and a hot rim along the glass's edges
  face += vec3(1.0, 0.45, 0.35)*halo(tr.x, 0.03 + px*1.2)*(0.45 + 0.8*pass + 0.6*uSwing);
  face += vec3(1.0, 0.5, 0.4)*halo(xs - ridge, 0.03 + px)*0.6;
  face += vec3(1.0, 0.2, 0.12)*pow(clamp(abs(xs)/max(a, 0.01), 0.0, 1.0), 6.0)*0.7 + vec3(1.0, 0.85, 0.8)*glint*0.8*step(0.6, id);
  // the seam's faces glow white-hot from the core when open
  face += vec3(1.0, 0.75, 0.65)*halo(x, 0.06 + px)*smoothstep(0.02, 0.1, g)*1.6*hc_open();
  face *= mix(0.25, 1.0, awake);
  float al = cov*mix(0.3, 0.92, awake);
  return vec4(mix(o.rgb, face, al), max(o.a, al));
}
// red smoke: soft puffs hugging the glass, pouring out of the crack while it is open; each
// puff faces the camera
vec3 hc_smoke(vec3 ro, vec3 rd){
  float t = uTime, op = hc_open(), top = hc_top(), ta;
  vec3 col = vec3(0.0);
  for (int i = 0; i < 20; i++){
    float fi = float(i);
    float rate = 0.16 + 0.1*h11(fi*2.3);
    float f = fract(t*rate + h11(fi*6.1));
    float k = floor(t*rate + h11(fi*6.1));
    float y0 = (0.08 + 0.84*h11(fi*3.9 + k))*top;
    float sd = h11(fi*1.1 + k) < 0.5 ? -1.0 : 1.0;
    float r = 0.7 + (0.8 + 3.0*op)*f;
    vec3 p = around(r, h11(fi*7.7 + k)*0.6 + (sd > 0.0 ? 0.1 : 0.6) + 0.04*t, y0 + 1.6*f);
    float dd = rayPt(ro, rd, p, ta);
    if (dd > 3.0) continue;
    float life = sin(3.1416*f)*(0.35 + 0.9*op);
    col += vec3(0.75, 0.05, 0.03)*(halo(dd, 0.5 + 0.7*f)*0.4 + halo(dd, 1.4)*0.08)*life;
  }
  return col;
}
// lightning: crawling off the glass and leaping out of the crack. front: 1 = only the bolts on
// the viewer's side of the crystal, 0 = all of them
vec3 hc_bolts(vec3 ro, vec3 rd, float px, float front){
  float L = uLength, t = uTime;
  float act = max(hc_open(), clamp(uSwing, 0.0, 1.0));
  vec3 light = vec3(0.0);
  vec2 toCam = -normalize(rd.xz + vec2(1e-5));
  for (int i = 0; i < 5; i++){
    float fi = float(i);
    float k = floor(t*10.0 + fi*0.37);
    float live = step(0.5 - 0.4*act, h11(k*1.3 + fi*7.0));
    if (live < 0.5) continue;
    float y0 = (0.08 + 0.84*h11(k*0.7 + fi*3.1))*hc_top();
    float an = h11(k + fi*2.0);
    vec3 a = around(HC_A*0.9 + hc_gap(y0), an, y0);
    vec3 c = around(HC_A + 1.8 + 3.0*act*h11(k*2.3 + fi), an + 0.12*(h11(k*5.0 + fi) - 0.5), y0 + (h11(k*3.7 + fi) - 0.35)*4.0);
    float facing = step(0.0, dot(c.xz, toCam));
    if (front > 0.5 && facing < 0.5) continue;
    float d = bolt3(ro, rd, a, c, k + fi*11.0, 0.38, px);
    light += (boltLight(d, vec3(1.0, 0.1, 0.05), px)*0.5 + vec3(1.0, 0.1, 0.05)*halo(d, 0.35)*0.5)*(0.4 + 0.9*act);
  }
  return light;
}
vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, top = hc_top();
  float op = hc_open();
  vec3 red = vec3(1.0, 0.1, 0.05);
  vec3 light = vec3(0.0);
  if (uFx < 0.01) return light;
  float s;
  float d = raySegRaw(ro, rd, vec3(0.0, 0.0, 0.0), vec3(0.0, top, 0.0), s);
  // the light spilling from the crack while it is open
  light += vec3(1.0, 0.25, 0.12)*halo(d, 0.6 + 1.4*op)*op*0.6*sin(3.1416*clamp(s, 0.0, 1.0));
  light += hc_smoke(ro, rd) + hc_bolts(ro, rd, px, 0.0);
  // clash: a triangular shockwave, the holocron's face, rushing out from the struck point,
  // facing the camera
  if (uClash > 0.01){
    float a = 1.0 - uClash;
    float hy = L*0.6, tc;
    float dc = rayPtRaw(ro, rd, vec3(0.0, hy, 0.0), tc);
    vec3 off = ro + rd*tc - vec3(0.0, hy, 0.0);
    vec3 ux = normalize(cross(rd, vec3(0.0, 1.0, 0.0)) + vec3(1e-4, 0.0, 0.0));
    vec3 uy = cross(ux, rd);
    vec2 q = vec2(dot(off, ux), dot(off, uy));
    float R = 1.5 + 6.0*pow(a, 0.6);
    float tri = max(max(q.y, dot(q, vec2(0.866, -0.5))), dot(q, vec2(-0.866, -0.5))) - R*0.5;
    light += red*(halo(abs(tri), 0.15 + 0.3*a)*1.8 + smoothstep(0.0, -1.5, tri)*0.25)*uClash;
    light += red*halo(dc, 1.3)*pow(uClash, 3.0)*1.5;
  }
  return light;
}
// the blade: Sol's glow and core from the file, beating with the heart, flaring when open
vec3 sig_glow_own(vec3 g, vec2 b, float r, float px){ return g*(0.8 + 0.5*hc_beat() + 0.8*hc_open()); }
vec3 sig_core(vec3 c, vec2 b, float px){ return c*(1.0 + 0.5*hc_open()); }

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxArcs(ro, rd, px, y0, y1, 5.0, 5.0, 1.7, 2.7, vec3(1.0, 0.12, 0.08))*part(1.0);
  return col + fx*uFx;
}

// round 15 (Creyon: some blades glowed so much their effects hardly showed): its glow at
// 80 %, so what is drawn round it reads
vec3 sig_glow(vec3 g, vec2 b, float r, float px){ return sig_glow_own(g, b, r, px)*0.8; }
