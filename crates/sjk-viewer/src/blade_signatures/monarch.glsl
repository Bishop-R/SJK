// Felucia Spores: the glowing fungal jungle of Felucia as a blade. A warm amber-green blade
// breathing with bioluminescent rings that pulse up it; glowing puffball spores rest on it while
// its wearer stands idle, lift off and swarm round it, following it, when they run or swing, and
// fly back to settle on it once they stand still (round 16); luminous filaments wave off its surface and now and then a spore pod bursts off it
// in a puff of light that rains spores. A swing brightens the spores; a clash bursts a huge cloud
// of them.
// @part Spores and filaments
// @room 10
// @palette {"corona.rim_cool":[0.30,0.35,0.02],"corona.rim_hot":[0.95,0.80,0.15],"corona.inner":[1.0,0.95,0.60],"core.white":[1.0,0.98,0.85],"core.fringe_cool":[0.70,0.85,0.20],"core.fringe_hot":[1.0,0.92,0.50],"light.color":[0.90,0.85,0.30],"trail":[0.90,0.85,0.35]}

float fs_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
vec3 fs_col(float k){ return k < 0.33 ? vec3(1.0, 0.75, 0.15) : k < 0.66 ? vec3(0.7, 1.0, 0.25) : vec3(0.25, 0.95, 0.8); }
vec3 base_vol(vec3 ro, vec3 rd, float px){
  float t = uTime, top = fs_top(), s, ta;
  float sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  vec3 col = vec3(0.0);
  if (top < 0.5) return col;
  // drifting spore dust round it all the time
  col += fxMotes(ro, rd, px, top*0.1, top, 1.0, 5.0, 24.0, 0.05, 2.7, vec3(0.8, 1.0, 0.4));
  // luminous filaments waving off the blade
  for (int i = 0; i < 10; i++){
    float fi = float(i);
    float y0 = (0.08 + 0.85*h11(fi*2.9))*top;
    float a = h11(fi*6.1);
    // 31/10/2026: a filament keeps within 2 of the axis between y0 - 0.4 and y0 + 1.2; its widest
    // glow is 0.18 (and a pixel's): a ray further off than three of them skips its five pieces
    if (fxNear(ro, rd, y0 - 0.4, y0 + 1.2) - 2.0 > 0.6 + 3.0*px) continue;
    vec3 prev = around(0.4, a, y0);
    float best = 1e3, bu = 0.0;
    for (int j = 1; j <= 5; j++){
      float u = float(j)/5.0;
      vec3 q = around(0.4 + 1.6*u, a + 0.06*sin(t*1.5 + fi + u*3.0), y0 + 0.8*u + 0.4*sin(t*2.0 + fi*1.7 + u*4.0)*u);
      float d = raySeg(ro, rd, prev, q, s);
      if (d < best){ best = d; bu = (float(j) - 1.0 + s)/5.0; }
      prev = q;
    }
    col += fs_col(h11(fi*4.4))*(halo(best, max(0.03, px))*0.9*(1.0 - bu) + halo(best, 0.18)*0.15) + vec3(1.0, 1.0, 0.8)*halo(length(vec2(best, 0.0)), 0.06)*step(0.9, bu)*1.2;
  }
  // spore pods bursting off it in a puff of light that rains spores
  for (int i = 0; i < 2; i++){
    float fi = float(i);
    float f = fract(t*0.3 + fi*0.5);
    float k = floor(t*0.3 + fi*0.5);
    vec3 c = around(0.6 + 1.0*f, h11(k*3.7 + fi), (0.2 + 0.6*h11(k*1.9 + fi))*top);
    float d = rayPt(ro, rd, c, ta);
    col += vec3(1.0, 0.95, 0.6)*(halo(d, 0.4 + 2.0*f)*0.5*(1.0 - f) + halo(abs(d - 3.0*f), 0.15)*0.6*(1.0 - f));
  }
  // a clash: a huge cloud of spores
  if (cl > 0.01){
    for (int i = 0; i < 16; i++){
      float fi = float(i);
      vec3 dir = normalize(vec3(sin(fi*2.4), 0.8*(h11(fi*3.0) - 0.4), cos(fi*2.4)));
      vec3 p = vec3(0.0, uLength*0.6, 0.0) + dir*(0.6 + 5.0*(1.0 - cl));
      float d = rayPt(ro, rd, p, ta);
      col += fs_col(h11(fi))*halo(d, max(0.08, px))*2.0*cl;
    }
  }
  // round 9: the jungle's light: soft beams of bioluminescence and a living aura
  float y0 = top*0.2, y1 = top*0.97;
  col += fxAura(ro, rd, y0, y1, 3.0, vec3(0.8, 0.9, 0.3));
  return col*uFx;
}
// the blade: Sol's glow and core from the file, bioluminescent rings pulsing up it
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float ring = pow(0.5 + 0.5*sin(b.y*0.9 - uTime*3.0), 6.0);
  return g*(0.85 + 0.6*ring + 0.5*clamp(uClash, 0.0, 1.0));
}
vec3 sig_core(vec3 c, vec2 b, float px){ return c; }
// round 11: sig_vol gated by part 0
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  if (part(0.0) < 0.5) { return vec3(0.0); }
  // round 15 (Creyon): no longer a chroma (it hid the spores' colours); its own colours
  return base_vol(ro, rd, px);
}
// the spores circle the blade, so they are drawn in true 3D round it (`sig_orbit`, Sol's way)
vec3 fs_spores(vec3 ro, vec3 rd, float px){
  float t = uTime, top = fs_top(), s, ta;
  float sw = clamp(uSwing, 0.0, 1.0);
  vec3 col = vec3(0.0);
  if (top < 0.5 || fxNear(ro, rd, 0.0, top) > 6.5) return col;
  // spores (round 16, Creyon): they rest on the blade while its wearer stands idle; running or
  // swinging, they lift off and swarm round it, following it; standing still again they fly
  // back and settle on it. `uSince` carries how stirred the blade is, 0 to 1, smooth both ways
  // (`saber_motion.rs` `stir`, `blade_lab_designs.rs` `STIRRED`); each spore leaves and lands at
  // its own moment of it. Each is a puffball: a bright heart in a soft fuzzy ball with a fine
  // ring, trailing a little light when it flies.
  float stir = uSince <= 1.0 ? clamp(uSince, 0.0, 1.0) : 0.0;
  for (int i = 0; i < 40; i++){
    float fi = float(i);
    float hs = h11(fi*6.3);
    float e = smoothstep(hs*0.45, hs*0.45 + 0.55, stir);
    e = e*e*(3.0 - 2.0*e);
    float yr = (0.06 + 0.9*h11(fi*3.1))*top;
    float ar = h11(fi*1.3);
    float rest = 0.6 + 0.12*h11(fi*2.7) + 0.04*sin(t*1.7 + fi*2.1);
    float sgn = h11(fi*9.3) < 0.5 ? -1.0 : 1.0;
    float spin = (0.12 + 0.2*h11(fi*5.1))*sgn;
    float rf = 1.6 + 3.0*h11(fi*7.7);
    vec3 fly = around(rf + 0.3*sin(t*1.1 + fi), ar + t*spin, yr + 1.2*sin(t*0.7 + fi*1.9) + 0.6);
    vec3 fly0 = around(rf + 0.3*sin(t*1.1 - 0.2 + fi), ar + (t - 0.25)*spin, yr + 1.2*sin((t - 0.25)*0.7 + fi*1.9) + 0.6);
    vec3 rest3 = around(rest, ar, yr);
    vec3 p = mix(rest3, fly, e);
    vec3 p0 = mix(rest3, fly0, e);
    // 31/10/2026: its widest glow is 0.55 (its ring 0.04 round less than 0.4): a ray further than
    // three of them from the spore and its trail draws nothing of it
    if (rayPtRaw(ro, rd, p, ta) - length(p - p0) > 1.7 + 3.0*px) continue;
    float d = raySeg(ro, rd, p0, p, s);
    float dp = rayPt(ro, rd, p, ta);
    float tw = 0.7 + 0.3*sin(t*(1.5 + 1.5*h11(fi)) + fi*2.0);
    vec3 sc = fs_col(h11(fi*8.8));
    float size = 0.16 + 0.08*h11(fi*4.4);
    col += (vec3(1.0)*halo(dp, max(0.05, px))*(0.9 + 0.6*tw) + sc*halo(dp, max(size, px))*(1.3 + tw)
            + sc*halo(abs(dp - size*1.6), 0.04)*0.7 + sc*halo(dp, 0.55)*0.25
            + sc*halo(d, max(0.05, px))*0.7*e)*(1.0 + 0.4*sw);
  }
  return col;
}
vec3 sig_orbit(vec3 ro, vec3 rd, float px){
  if (part(0.0) < 0.5) { return vec3(0.0); }
  return fs_spores(ro, rd, px)*uFx;
}
