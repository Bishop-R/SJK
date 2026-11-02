// Exegol Rift: the hidden Sith world of Exegol torn open in a blade. A dark rift runs down the
// blade's heart between glowing red-violet edges; every few seconds the rift tears wide at one
// place and spills violet light; slow, kinked Sith lightning leaps from the tip and crawls
// down, a dark storm churns round it and dust is pulled up into the rift. Round 16 (Creyon: now a
// mythical, make it even better): a vortex of the dark side whirls round its upper half in 3D,
// three ribbons of black-violet energy with burning crimson edges spiralling up out of the rift,
// and Sith lightning leaps from the vortex into the blade now and then. A clash flares it.
// @part Rift tear
// @part Storm clouds
// @part Rising dust
// @arcs Sith lightning
// @room 8
// @palette {"corona.rim_cool":[0.08,0.1,0.35],"corona.rim_hot":[0.5,0.65,1.0],"corona.inner":[0.06,0.06,0.18],"core.white":[0.35,0.4,0.7],"core.fringe_cool":[0.3,0.4,0.9],"core.fringe_hot":[0.65,0.75,1.0],"light.color":[0.45,0.55,1.0],"trail":[0.45,0.55,1.0]}

vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float heart = exp(-pow(b.x/0.22, 2.0));
  float pulse = 0.7 + 0.3*sin(uTime*1.4 - b.y*0.25);
  vec3 rim = vec3(1.0, 0.25, 0.7)*halo(abs(abs(b.x) - 0.24), 0.07)*pulse*part(0.0);
  return g*(1.0 - 0.75*heart*part(0.0)) + lvl(g)*rim*1.3 + g*0.5*clamp(uClash, 0.0, 1.0);
}
vec3 sig_core(vec3 c, vec2 b, float px){
  // the rift's dark heart: the white core darkened to a deep violet, its edges hot
  float edge = smoothstep(0.0, 1.0, abs(b.x)/0.18);
  return mix(c*vec3(0.25, 0.08, 0.35), c*vec3(1.0, 0.6, 1.0), edge*0.6)*part(0.0) + c*(1.0 - part(0.0));
}
// the effects round the blade, in three dimensions
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  vec3 v = vec3(0.75, 0.3, 1.0), red = vec3(1.0, 0.2, 0.45);
  // the tear: every few seconds the rift rips wide at one place and spills violet light
  float k = floor(t/3.5), q = fract(t/3.5);
  float yt = mix(y0 + 3.0, y1 - 5.0, h11(k*7.3));
  float open_ = smoothstep(0.0, 0.15, q)*(1.0 - smoothstep(0.45, 0.9, q));
  fx += fxAura(ro, rd, yt - 2.5 - 2.0*open_, yt + 2.5 + 2.0*open_, 0.9 + 1.4*open_, v)*open_*2.6*part(0.0);
  fx += fxFlare(ro, rd, vec3(0.0, yt, 0.0), px, 0.25, 2.0 + 3.0*open_, 2.0, 1.5708, vec3(0.9, 0.5, 1.0))*open_*part(0.0);
  // a dark storm churning round it
  fx += fxSmoke(ro, rd, y0, y1, 4.2, 0.6, 3.8, vec3(0.22, 0.1, 0.32))*1.1*part(1.0);
  fx += fxSmoke(ro, rd, y0, y1, 2.6, -0.8, 7.1, vec3(0.35, 0.08, 0.25))*0.8*part(1.0);
  // dust pulled up into the rift
  fx += fxFall(ro, rd, px, y0, y1, 2.6, 18.0, -6.0, 0.5, 0.0, 6.6, red)*0.6*part(2.0);
  return fx*uFx;
}
// round 16: the dark side's vortex whirling round the blade's upper half, ribbons of energy
// spiralling in toward it with crimson edges, and bolts leaping from it into the blade; it
// circles the blade, so it is drawn in true 3D (Sol's way); with the storm (part 1)
vec3 sig_orbit(vec3 ro, vec3 rd, float px){
  if (part(1.0) < 0.5) { return vec3(0.0); }
  float L = uLength, t = uTime, s;
  vec3 fx = vec3(0.0);
  float yc = L*0.68;
  if (fxNear(ro, rd, yc - 8.0, yc + 6.0) > 6.5) return fx;
  vec3 v = vec3(0.75, 0.3, 1.0), red = vec3(1.0, 0.15, 0.35);
  for (int j = 0; j < 3; j++){
    float fj = float(j);
    float best = 1e3, bs = 0.0;
    vec3 prev = vec3(0.0);
    for (int i = 0; i <= 18; i++){
      float u = float(i)/18.0;
      // a funnel opening up out of the rift: narrow at the blade, wide above
      float R = mix(0.6, 4.4, u*u);
      vec3 p = around(R, u*1.3 + fj/3.0 - t*0.22, yc - 5.0 + 9.0*u);
      if (i > 0){ float d = raySeg(ro, rd, prev, p, s); if (d < best){ best = d; bs = (float(i) - 1.0 + s)/18.0; } }
      prev = p;
    }
    float flow = 0.6 + 0.4*sin(bs*20.0 + t*5.0 + fj*2.0);
    float ends = smoothstep(0.0, 0.1, bs)*(1.0 - smoothstep(0.9, 1.0, bs));
    fx += (red*halo(best, max(0.04, px))*1.4*flow + v*halo(best, 0.3 + 0.2*(1.0 - bs))*0.6 + vec3(0.25, 0.05, 0.4)*halo(best, 1.1)*0.25)*ends;
  }
  // a bolt from the vortex into the blade now and then
  float cyc = t*0.7, k = floor(cyc), q = fract(cyc);
  if (q < 0.25){
    float a = h11(k*3.1);
    vec3 from = around(3.5, a - t*0.22, yc - 3.0 + 2.0*h11(k*5.3));
    vec3 to = vec3(0.0, yc - 1.0 + 3.0*h11(k*7.7), 0.0);
    float d = bolt3(ro, rd, from, to, k*1.7 + floor(t*20.0)*0.3, 0.45, px);
    fx += boltLight(d, vec3(0.85, 0.45, 1.0), px)*(1.0 - q/0.25)*0.9;
  }
  return fx*uFx;
}
