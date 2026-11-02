// Reactor: a ship's hypermatter reactor core as a blade (Creyon's round 9, rebuilt as Sol builds
// his). The blade is Sol's glow and core from the file, squeezed by magnetic containment rings
// that march steadily up it in 3D and fade out softly at the tip. Glowing magnetic loops arch
// between the rings, with plasma beads riding them; a haze of reactor dust and smoke hangs round
// it. Over a 30 second cycle it heats: calm blue, then orange, then critical red with a strobe,
// plasma venting and lightning crackling off it, until a containment flash throws a shock ring
// and beams of light, and it cools again. A swing stretches the loops and vents harder; a clash
// is a containment breach.
// @part Containment and venting
// @wisps Reactor smoke
// @room 10
// @palette {"corona.rim_cool":[0.08,0.3,0.9],"corona.rim_hot":[0.35,0.7,1.0],"corona.inner":[0.4,0.7,1.0],"core.white":[0.7,0.88,1.0],"core.fringe_cool":[0.2,0.55,1.0],"core.fringe_hot":[0.7,0.88,1.0],"light.color":[0.4,0.75,1.0],"trail":[0.4,0.75,1.0]}

const float RC_N = 5.0;     // rings on the blade at once
const float RC_V = 0.1;     // ring spacings a second
// the heat cycle: x heating (0 calm .. 1 hot), y critical, z the flash
vec3 rc_heat(){
  float cyc = mod(uTime, 30.0);
  float cooling = 1.0 - smoothstep(26.0, 30.0, cyc);
  float hx = smoothstep(16.0, 22.0, cyc)*cooling;
  float hy = smoothstep(21.6, 22.4, cyc)*(1.0 - smoothstep(25.6, 26.2, cyc));
  float hz = exp(-max(cyc - 26.0, 0.0)*2.0)*step(26.0, cyc);
  return vec3(hx, hy, hz);
}
vec3 rc_col(vec3 h){ return mix(mix(vec3(0.3, 0.7, 1.0), vec3(1.0, 0.45, 0.12), h.x), vec3(1.0, 0.18, 0.06), h.y); }
// ring k's height: marching up, wrapping round from the hilt
float rc_ringY(float k){ return fract((k + uTime*RC_V*RC_N)/RC_N)*(uLength + 2.0) - 1.0; }
// how much ring k shows: fading in above the hilt and out softly before the tip (never popping)
float rc_ringVis(float y){ return smoothstep(0.5, 3.0, y)*(1.0 - smoothstep(uLength - 3.5, uLength + 0.5, y)); }

vec3 base_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  vec3 h = rc_heat();
  vec3 pc = rc_col(h);
  float strobe = h.y*(0.5 + 0.5*step(0.5, fract(t*2.0)));
  vec3 col = vec3(0.0);
  float near = fxNear(ro, rd, 0.0, L);
  if (near > 10.0) return col;
  float s;
  // ---- the containment rings: thin bright hoops round the blade, marching up ----
  for (int k = 0; k < 5; k++){
    float y = rc_ringY(float(k));
    float vis = rc_ringVis(y);
    if (vis <= 0.0) continue;
    float R = 1.15*(1.0 + 0.12*h.x*sin(t*5.0));
    float d = ringD(ro, rd, y, R);
    col += (vec3(1.0)*halo(d, max(0.05, px))*1.2 + pc*halo(d, 0.25)*0.8 + pc*halo(d, 0.8)*0.15)*vis*(1.0 + strobe);
  }
  // ---- magnetic loops arching between neighbouring rings on four sides, plasma beads riding ----
  for (int k = 0; k < 5; k++){
    float ya = rc_ringY(float(k)), yb = ya + (L + 2.0)/RC_N;
    float vis = min(rc_ringVis(ya), rc_ringVis(yb));
    if (vis <= 0.0) continue;
    float H = (2.2 + 1.6*h.x + 2.0*sw + 2.5*cl)*(0.85 + 0.15*sin(t*3.0 + float(k)*1.7));
    for (int j = 0; j < 4; j++){
      float a = float(j)*0.25 + t*0.03 + float(k)*0.11;
      // 31/10/2026: the loop lies within H/2 + (yb - ya)/2 of the middle of its span; its widest
      // glow is 0.22 (and a pixel's), under a ten-thousandth three of them off: a ray further
      // off skips its eight pieces
      float tm;
      if (rayPtRaw(ro, rd, around(1.15 + 0.5*H, a, 0.5*(ya + yb)), tm) - 0.5*H - 0.5*(yb - ya) > 0.7 + 3.0*px) continue;
      float best = 1e3, bs = 0.0;
      vec3 prev = around(1.15, a, ya);
      for (int i = 1; i <= 8; i++){
        float f = float(i)/8.0;
        vec3 q = around(1.15 + H*sin(3.1416*f), a, mix(ya, yb, f));
        float d = raySeg(ro, rd, prev, q, s);
        if (d < best){ best = d; bs = (float(i) - 1.0 + s)/8.0; }
        prev = q;
      }
      float bead = pow(0.5 + 0.5*sin(bs*12.0 - t*6.0 + float(j)), 8.0);
      col += (vec3(1.0)*halo(best, max(0.04, px))*(0.3 + 1.5*bead) + pc*halo(best, 0.22)*(0.45 + 0.6*bead))*vis*(0.5 + 0.5*h.x + 0.6*sw);
    }
  }
  float y0 = L*0.15, y1 = L*0.97;
  // ---- venting plasma and lightning as it heats ----
  col += fxSparks(ro, rd, px, y0, y1, 6.0 + 22.0*max(h.x, sw), 6.0, 1.2, 2.2, pc)*(0.3 + h.x + sw);
  // round 23 (Sol's review, timed on Creyon's RTX 5070): the lightning is worked out only while
  // it shows (cold and still, more than half of its 30-second cycle, it is weighted to nothing)
  float arcs = max(h.x, 0.25*sw);
  if (arcs > 0.0) col += fxArcs(ro, rd, px, y0, y1, 1.0 + 6.0*h.x + 2.0*sw, 4.5, 2.0 + 2.0*h.y, 5.3, pc)*arcs;
  // ---- reactor dust and a glowing haze round it ----
  col += fxMotes(ro, rd, px, y0, y1, 1.0, 5.5, 30.0, 0.06, 3.3, mix(pc, vec3(1.0), 0.3));
  col += fxAura(ro, rd, y0, y1, 3.0, pc)*(0.8 + 0.6*h.x + strobe);
  // ---- the containment flash: a shock ring and beams of light; a clash is a breach ----
  float fq = mod(t, 30.0) - 26.0;
  // round 19 (Creyon: the ring was cut off where it left the blade's room): it races out to
  // well inside that room and fades there
  col += fxShock(ro, rd, px, L*0.5, fq/1.6, 6.5, vec3(1.0, 0.9, 0.8))*2.0;
  if (h.z > 0.0) col += fxSparks(ro, rd, px, y0, y1, 24.0, 7.0, 1.5, 9.1, pc)*h.z*2.0;
  return col*uFx;
}

// the blade: Sol's glow and core from the file, turned to the reactor's heat and pinched brighter
// where each ring squeezes it
float rc_pinch(float y){
  float p = 0.0;
  for (int k = 0; k < 5; k++){ float yr = rc_ringY(float(k)); p += halo(y - yr, 0.6)*rc_ringVis(yr); }
  return p;
}
vec3 sig_glow_own(vec3 g, vec2 b, float r, float px){
  vec3 h = rc_heat();
  float l = max(g.r, max(g.g, g.b));
  float strobe = h.y*(0.5 + 0.5*step(0.5, fract(uTime*2.0)));
  vec3 c = mix(g, l*rc_col(h)*1.1, max(h.x, h.y));
  return c*(1.0 + 0.6*rc_pinch(b.y) + 0.4*strobe + 0.8*h.z + 0.5*clamp(uClash, 0.0, 1.0));
}
vec3 sig_core(vec3 c, vec2 b, float px){
  vec3 h = rc_heat();
  return mix(c, vec3(1.0, 0.85, 0.75)*max(c.r, max(c.g, c.b)), h.y*0.5)*(1.0 + 0.4*rc_pinch(b.y) + 0.6*h.z);
}
// round 11: sig_vol gated by part 0
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  if (part(0.0) < 0.5) { return vec3(0.0); }
  return base_vol(ro, rd, px);
}

// round 15 (Creyon: some blades glowed so much their effects hardly showed): its glow at
// 60 %, so what is drawn round it reads
vec3 sig_glow(vec3 g, vec2 b, float r, float px){ return sig_glow_own(g, b, r, px)*0.6; }
