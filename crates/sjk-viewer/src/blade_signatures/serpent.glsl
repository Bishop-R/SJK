// Sith Alchemy: the dark science of the Sith as a blade. An emerald blade veined with gold
// corruption that crawls up it and pulses; green alchemical fire licks off it in tall tongues,
// sickly green lightning crackles from it, dark-green vapour coils round it and motes of gold
// dust drift in the air. Every few seconds the veins flare and a wave of green fire rolls up
// the blade. A swing feeds the fire; a clash bursts a ring of green flame and lightning.
// @part Alchemy fire
// @part Gold lightning
// @arcs Green lightning
// @motes Gold dust
// @room 10
// @palette {"corona.rim_cool":[0.02,0.30,0.10],"corona.rim_hot":[0.20,1.0,0.45],"corona.inner":[0.75,1.0,0.80],"core.white":[0.95,1.0,0.92],"core.fringe_cool":[0.15,0.90,0.40],"core.fringe_hot":[0.70,1.0,0.60],"light.color":[0.25,1.0,0.45],"trail":[0.30,1.0,0.50]}

float sa_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
float sa_wave(float y){ float f = fract(uTime/4.0); return halo(y - f*(uLength + 8.0) + 4.0, 2.5)*step(0.0, f); }
vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float t = uTime, top = sa_top(), s, ta;
  float sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  vec3 green = vec3(0.2, 1.0, 0.4), gold = vec3(1.0, 0.8, 0.3), pale = vec3(0.8, 1.0, 0.75);
  vec3 col = vec3(0.0);
  if (top < 0.5) return col;
  // green fire licking off it in tall tongues
  for (int i = 0; i < 16; i++){
    float fi = float(i);
    float rate = 0.8 + 0.5*h11(fi*2.1);
    float f = fract(t*rate + h11(fi*4.3));
    float k = floor(t*rate + h11(fi*4.3));
    float y0 = (0.03 + 0.9*h11(fi*3.7 + k))*top;
    float a0 = h11(fi*5.9 + k) + t*0.1;
    float H = (1.8 + 1.8*h11(fi*1.1 + k))*(1.0 + 0.5*sw + 1.2*sa_wave(y0));
    vec3 p0 = around(0.4, a0, y0 + 1.5*f);
    vec3 p1 = around(0.5 + 0.6*H*f, a0 + 0.06*sin(t*5.0 + fi), y0 + 1.5*f + H);
    float d = raySeg(ro, rd, p0, p1, s);
    float w = max(0.3*(1.0 - s)*(1.0 - 0.5*f) + 0.04, px);
    col += (mix(pale, green, s)*halo(d, w)*1.3 + green*halo(d, w*2.5)*0.3)*sin(3.1416*f)*(1.0 - 0.6*s);
  }
  // sickly green lightning crackling off it
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float k = floor(t*(6.0) + fi*0.37);
    if (h11(k*1.9 + fi) < 0.4 - 0.3*sw) continue;
    float y0 = (0.08 + 0.8*h11(k*2.3 + fi))*top;
    float a = h11(k*4.7 + fi);
    float reach = 2.5 + 2.5*h11(k*1.1 + fi);
    float bd = bolt3(ro, rd, around(0.4, a, y0), around(reach, a + 0.08, y0 + 2.0*(h11(k*3.3) - 0.3)), k + fi*13.0, 0.4, px);
    col += boltLight(bd, green, px)*1.2;
  }
  // dark-green vapour coiling round it and gold dust in the air
  for (int i = 0; i < 10; i++){
    float fi = float(i);
    float f = fract(t*0.15 + fi/10.0);
    vec3 p = around(1.2 + 1.2*f, fi*0.31 + f*0.6, (0.1 + 0.8*h11(fi*2.2 + floor(t*0.15 + fi/10.0)))*top + 2.0*f);
    float d = rayPt(ro, rd, p, ta);
    col += vec3(0.05, 0.35, 0.12)*halo(d, 0.6 + 0.7*f)*0.35*sin(3.1416*f);
  }
  for (int i = 0; i < 20; i++){
    float fi = float(i);
    vec3 p = around(1.0 + 3.5*h11(fi*3.9), h11(fi*7.3) + t*0.02, mod(h11(fi*5.1)*uLength + t*(0.3 + 0.3*h11(fi)), top + 2.0) - 1.0);
    float d = rayPt(ro, rd, p, ta);
    col += gold*halo(d, max(0.04, px))*1.2*(0.5 + 0.5*sin(t*4.0 + fi*2.0));
  }
  // a clash: a ring of green flame and a burst of lightning
  if (cl > 0.01){
    float yc = uLength*0.6;
    vec3 h = rayAt(ro, rd, yc);
    if (h.z > 0.0) col += green*halo(hideD(abs(length(h.xy) - 0.6 - 5.0*(1.0 - cl)), behindBlade(ro, rd, h.z)), 0.25 + 0.4*(1.0 - cl))*cl*1.8;
    float dc = rayPtRaw(ro, rd, vec3(0.0, yc, 0.0), ta);
    col += pale*halo(dc, 1.5)*cl*cl*1.5;
  }
  return col*uFx;
}
// the blade: Sol's glow and core from the file veined with gold corruption that crawls up it
// and flares with the wave of green fire
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float v = halo(abs(vn(vec2(b.x*2.5, b.y*0.5 - uTime*0.4)) - 0.5), 0.04);
  float w = sa_wave(b.y);
  float on = smoothstep(0.0, 0.5, b.y)*(1.0 - smoothstep(uLength - 1.0, uLength, b.y))*smoothstep(1.2, 0.2, abs(b.x));
  return g*(0.85 + 0.8*w + 0.5*clamp(uClash, 0.0, 1.0)) + vec3(1.0, 0.8, 0.3)*v*on*(0.6 + 1.2*w);
}
vec3 sig_core(vec3 c, vec2 b, float px){ return c; }

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxArcs(ro, rd, px, y0, y1, 3.0, 3.5, 1.2, 8.2, vec3(1.0, 0.8, 0.3))*part(1.0);
  return col + fx*uFx;
}
