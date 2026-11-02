// World Shift: a blade that carries the climates of the galaxy's worlds, six seconds each: Mustafar
// (a molten blade under a cracked black crust, running with lava drips and fire), Hoth (a faceted
// ice crystal with frost spikes in a swirling blizzard), Kamino (a storm blade wrapped in lightning
// with churning sea water, storm cloud and driving rain), Tatooine (a golden blade in a whirling
// vortex of sand and wind), Crait (white salt crust cracking to show the red beneath, rock shards
// drifting round it) and Mon Cala (a flowing column of water with spiralling streams and spray).
// Each change is an event: the old climate strains, then bursts off the blade in a flash with shock
// rings and its debris flung out all round, and the new one condenses onto it. Swings stoke it;
// a clash flashes the ruling climate.
// @part Climates
// @part Sparks
// @room 10
// @raycache off
// @solid 1
// @palette {"corona.rim_cool":[0.35,0.20,0.55],"corona.rim_hot":[0.90,0.60,0.50],"corona.inner":[1.0,0.92,0.85],"core.white":[1.0,0.98,0.95],"core.fringe_cool":[0.50,0.70,1.0],"core.fringe_hot":[1.0,0.65,0.35],"light.color":[0.95,0.75,0.60],"trail":[0.90,0.70,0.60]}


// ---- the cycle: 0 Mustafar, 1 Hoth, 2 Kamino, 3 Tatooine, 4 Crait, 5 Mon Cala ----
const float WS_P = 6.0;
float ws_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
float ws_cur(){ return mod(floor(uTime/WS_P), 6.0); }
float ws_prev(){ return mod(floor(uTime/WS_P) + 5.0, 6.0); }
float ws_lt(){ return fract(uTime/WS_P)*WS_P; }
// did the last change happen while the blade was lit? (no burst right after ignition)
float ws_boom(){ return uAge > ws_lt() + 0.05 ? 1.0 : 0.0; }
// the new climate condensing onto the blade (0..1)
float ws_form(){ return ws_boom() > 0.5 ? smoothstep(0.12, 1.1, ws_lt()) : 1.0; }
// the ruling climate straining just before it bursts
float ws_charge(){ return smoothstep(WS_P - 0.8, WS_P, ws_lt()); }
vec3 ws_glow(float k){
  return k < 0.5 ? vec3(1.0, 0.36, 0.05) : k < 1.5 ? vec3(0.45, 0.8, 1.0) : k < 2.5 ? vec3(0.35, 0.55, 1.0)
       : k < 3.5 ? vec3(1.0, 0.66, 0.25) : k < 4.5 ? vec3(1.0, 0.14, 0.1) : vec3(0.15, 0.62, 0.95);
}
// a blade body: white-hot core, coloured glow
vec3 ws_core(float d, float px, float w, vec3 hot, vec3 glow, float gain){
  // the blade itself is Sol's glow and core from the file (sig_glow, sig_core)
  return vec3(0.0);
}
// the ray's distance to a ring of radius R round the blade at height y
float ws_ring(vec3 ro, vec3 rd, float y, float R){
  float best = 1e3;
  vec3 h = rayAt(ro, rd, y);
  if (h.z > 0.0) best = hideD(abs(length(h.xy) - R), behindBlade(ro, rd, h.z));
  float a = dot(rd.xz, rd.xz), b = dot(ro.xz, rd.xz), c = dot(ro.xz, ro.xz) - R*R;
  float dsc = b*b - a*c;
  if (dsc > 0.0 && a > 1e-6){
    float q = sqrt(dsc);
    float t0 = (-b - q)/a, t1 = (-b + q)/a;
    if (t0 > 0.0) best = min(best, hideD(abs(ro.y + rd.y*t0 - y), behindBlade(ro, rd, t0)));
    if (t1 > 0.0) best = min(best, hideD(abs(ro.y + rd.y*t1 - y), behindBlade(ro, rd, t1)));
  }
  return best;
}
// fire colour by heat 0..1
vec3 ws_fire(float h){
  vec3 c = mix(vec3(0.6, 0.05, 0.0), vec3(1.0, 0.38, 0.04), smoothstep(0.0, 0.4, h));
  c = mix(c, vec3(1.0, 0.75, 0.3), smoothstep(0.4, 0.75, h));
  return mix(c, vec3(1.0, 0.95, 0.8), smoothstep(0.75, 1.0, h));
}
// a wind streak: an arc of 5 pieces sweeping round the blade; returns its light weight
float ws_wind(vec3 ro, vec3 rd, float r, float a, float y, float span, float rise, float px){
  float best = 1e3, bf = 0.0, s;
  vec3 prev = around(r, a, y);
  for (int j = 1; j <= 5; j++){
    float f = float(j)/5.0;
    vec3 q = around(r*(1.0 + 0.08*f), a + span*f, y + rise*f);
    float dd = raySeg(ro, rd, prev, q, s);
    if (dd < best){ best = dd; bf = (float(j) - 1.0 + s)/5.0; }
    prev = q;
  }
  float taper = sin(3.1416*bf)*(0.4 + 0.6*bf);
  return (halo(best, max(0.035, px))*1.0 + halo(best, 0.2)*0.25)*taper;
}

// soft clouds of each climate round the blade: fire tongues (Mustafar), snow mist (Hoth),
// storm cloud (Kamino), sand (Tatooine), red dust (Crait) and spray (Mon Cala), each puff a
// soft piece facing the camera, drifting with its world's weather
vec3 ws_puffs(vec3 ro, vec3 rd, float k, float top, float form, float st, float ch){
  float t = uTime, ta;
  vec3 col = vec3(0.0);
  for (int i = 0; i < 20; i++){
    float fi = float(i);
    float rate = k < 0.5 ? 0.9 : k < 1.5 ? 0.25 : k < 2.5 ? 0.12 : k < 3.5 ? 0.35 : k < 4.5 ? 0.18 : 0.4;
    float f = fract(t*rate*(0.7 + 0.6*h11(fi*2.3)) + h11(fi*5.9));
    float cy = floor(t*rate*(0.7 + 0.6*h11(fi*2.3)) + h11(fi*5.9));
    float y0 = (0.03 + 0.92*h11(fi*3.7 + cy))*top;
    float r, a, yy, w;
    vec3 c;
    if (k < 0.5){
      // fire tongues licking up off the blade
      r = 0.6 + 1.6*f; a = h11(fi*1.3 + cy); yy = y0 + 3.0*f; w = 0.5*(1.0 - 0.5*f) + 0.25;
      c = ws_fire(1.0 - f*0.9)*1.8;
    } else if (k < 1.5){
      r = 1.0 + 2.6*h11(fi*4.1 + cy); a = h11(fi*1.3 + cy) + t*0.12; yy = y0 + 0.6*sin(t + fi); w = 0.9 + 0.6*h11(fi);
      c = vec3(0.6, 0.8, 1.0)*0.5;
    } else if (k < 2.5){
      r = 1.4 + 2.4*h11(fi*4.1 + cy); a = h11(fi*1.3 + cy) + t*0.05; yy = y0; w = 1.0 + 0.8*h11(fi);
      c = mix(vec3(0.12, 0.16, 0.3), vec3(0.5, 0.65, 1.0), 0.5 + 0.5*sin(t*9.0 + fi*3.0)*step(0.85, h11(floor(t*6.0) + fi)));
    } else if (k < 3.5){
      r = 0.9 + 3.0*f; a = h11(fi*1.3 + cy) + f*0.8; yy = y0 + 0.8*f; w = 0.6 + 0.6*f;
      c = vec3(1.0, 0.66, 0.3)*0.6;
    } else if (k < 4.5){
      r = 0.9 + 1.6*f; a = h11(fi*1.3 + cy) + 0.1*f; yy = y0 + 1.0*f; w = 0.5 + 0.5*f;
      c = vec3(0.8, 0.12, 0.08)*0.6;
    } else {
      r = 0.7 + 1.2*f; a = h11(fi*1.3 + cy) - f*0.6; yy = y0 + 1.6*f; w = 0.35 + 0.35*f;
      c = vec3(0.4, 0.8, 1.0)*0.6;
    }
    vec3 p = around(r, a, yy);
    float dd = rayPt(ro, rd, p, ta);
    if (dd > 3.5) continue;
    col += c*(halo(dd, w)*0.6 + halo(dd, w*2.2)*0.12)*sin(3.1416*f)*form*(1.0 + 0.5*st + 0.5*ch);
  }
  return col;
}
// ======== MUSTAFAR: molten blade, black crust (solid pass), fire, drips ========
vec3 ws_lava(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s, ta;
  vec3 col = ws_core(d, px, 0.42, vec3(1.0, 0.85, 0.5), vec3(1.0, 0.36, 0.05), 1.0 + ch + 0.3*st);
  col += ws_puffs(ro, rd, 0.0, top, form, st, ch)*fx;
  // molten drips running down the blade and dropping away from it
  for (int i = 0; i < 9; i++){
    float fi = float(i);
    float cyc = t*0.32 + h11(fi*3.1);
    float k = floor(cyc), f = fract(cyc);
    float y0 = (0.25 + 0.7*h11(k*1.9 + fi))*top;
    float a = h11(fi*5.3 + k);
    float off = max(f - 0.5, 0.0);
    float yy = y0 - 5.0*f*f, rr = 0.66 + 6.0*off*off;
    vec3 p1 = around(rr, a, yy), p0 = around(0.66 + 6.0*max(off - 0.08, 0.0)*max(off - 0.08, 0.0), a, yy + 0.6 + 1.2*f);
    float dd = raySeg(ro, rd, p0, p1, s);
    float life = smoothstep(0.0, 0.1, f)*(1.0 - smoothstep(0.75, 1.0, f))*form;
    float dh = rayPt(ro, rd, p1, ta);
    col += (vec3(1.0, 0.55, 0.12)*halo(dd, max(0.05, px))*1.2 + vec3(1.0, 0.8, 0.4)*halo(dh, max(0.11, px))*1.8 + vec3(1.0, 0.3, 0.03)*halo(dd, 0.45)*0.3)*life*fx;
  }
  // embers rising
  for (int i = 0; i < 10; i++){
    float fi = float(i);
    float f = fract(t*(0.2 + 0.1*h11(fi*2.9)) + h11(fi*5.1));
    vec3 p = around(0.8 + 3.0*h11(fi*3.7)*f, h11(fi*1.9) + 0.15*sin(t + fi), f*(top + 2.0));
    float dd = rayPt(ro, rd, p, ta);
    col += vec3(1.0, 0.5, 0.1)*(halo(dd, max(0.06, px))*1.8 + halo(dd, 0.3)*0.15)*smoothstep(0.0, 0.1, f)*(1.0 - smoothstep(0.7, 1.0, f))*form*fx;
  }
  return col;
}

// ======== HOTH: a hexagonal ice crystal, frost spikes, blizzard ========
vec3 ws_ice(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s, ta;
  vec3 col = ws_core(d, px, 0.32, vec3(0.9, 0.97, 1.0), vec3(0.4, 0.75, 1.0), 1.0 + ch + 0.3*st);
  float yc = -0.3 + (top + 0.3)*form;
  float jit = ch*0.04*sin(t*70.0);
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (fx <= 0.0 || far > 8.0) return col;
  // frost spikes jutting from the crystal
  for (int i = 0; i < 12; i++){
    float fi = float(i);
    float y0 = (0.06 + 0.86*h11(fi*2.7))*top;
    if (y0 > yc - 1.0) continue;
    float a = h11(fi*6.1) + fi*0.37;
    float ln = (0.5 + 1.1*h11(fi*1.3))*form*(1.0 + 0.3*st);
    vec3 b0 = around(0.55, a, y0), b1 = around(0.55 + ln, a + 0.03, y0 + ln*0.7);
    float dd = raySeg(ro, rd, b0, b1, s);
    col += (vec3(0.95, 1.0, 1.0)*halo(dd, max(0.04*(1.0 - s) + 0.012, px))*1.4 + vec3(0.4, 0.75, 1.0)*halo(dd, 0.25)*0.3)*fx;
  }
  // blizzard: wind streaks sweeping round the blade
  float wr = 1.0 + 2.5*(1.0 - form);
  for (int i = 0; i < 7; i++){
    float fi = float(i);
    float cyc = t*0.5 + h11(fi*4.3);
    float k = floor(cyc), f = fract(cyc);
    float r = (1.3 + 2.4*h11(fi*2.2 + k))*wr;
    float y = (0.05 + 0.85*h11(fi*1.7 + k*3.1))*top;
    float a = h11(fi*8.3 + k) + f*0.9;
    col += vec3(0.85, 0.93, 1.0)*ws_wind(ro, rd, r, a, y, 0.22, 1.2, px)*sin(3.1416*f)*(0.7 + 0.5*st)*form*fx;
  }
  // snow whirling round
  for (int i = 0; i < 30; i++){
    float fi = float(i);
    float r = (1.0 + 3.6*h11(fi*3.3))*wr;
    float a = h11(fi*7.7) + t*(0.35 + 0.25*h11(fi*1.1));
    float yy = mod(h11(fi*5.9)*(top + 2.0) - t*(1.2 + 1.5*h11(fi*2.4)), top + 2.0) - 1.0;
    float dd = rayPt(ro, rd, around(r, a, yy), ta);
    col += vec3(0.92, 0.97, 1.0)*(halo(dd, max(0.05, px))*1.4 + halo(dd, 0.2)*0.1)*form*fx;
  }
  // snow mist
  col += ws_puffs(ro, rd, 1.0, top, form, st, ch)*fx;
  return col;
}

// ======== KAMINO: lightning wrapping the blade, churning sea, storm cloud, rain ========
vec3 ws_storm(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s;
  float fl = 0.0;
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float k = floor(t*6.0 + fi*0.37);
    fl += step(0.3, h11(k*1.7 + fi))*exp(-fract(t*6.0 + fi*0.37)*4.0);
  }
  vec3 col = ws_core(d, px, 0.34, vec3(0.88, 0.93, 1.0), vec3(0.3, 0.55, 1.0), 1.0 + ch + 0.25*fl + 0.3*st);
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (fx <= 0.0 || far > 8.0) return col;
  // lightning wrapping round the blade in 3D
  if (far < 5.0){
    for (int i = 0; i < 3; i++){
      float fi = float(i);
      float k = floor(t*6.0 + fi*0.37);
      float on = step(0.3, h11(k*1.7 + fi))*(0.6 + 0.4*ch);
      if (on < 0.01) continue;
      float y0 = (0.06 + 0.72*h11(k*2.3 + fi*3.1))*top;
      float dy = (3.0 + 4.0*h11(k*3.9 + fi))*(0.6 + 0.4*form);
      float a = h11(k*5.1 + fi);
      float reach = (1.6 + 1.4*h11(k*0.7 + fi))*(1.0 + 0.4*st);
      vec3 A = around(0.35, a, y0), M = around(reach, a + 0.25, y0 + dy*0.5), C = around(0.35, a + 0.5, y0 + dy);
      float bd = min(bolt3(ro, rd, A, M, k + fi*11.0, 0.3, px), bolt3(ro, rd, M, C, k + fi*11.0 + 0.5, 0.3, px));
      col += boltLight(bd, vec3(0.45, 0.6, 1.0), px)*on*1.3*form;
    }
  }
  // the churning sea round the blade and the storm cloud beyond (marched)
  col += ws_puffs(ro, rd, 2.0, top, form, st, ch)*fx;
  // driving rain
  for (int i = 0; i < 26; i++){
    float fi = float(i);
    float r = (0.8 + 3.8*h11(fi*3.3))*(1.0 + 1.5*(1.0 - form));
    float a = h11(fi*7.7);
    float yy = top + 2.0 - mod(h11(fi*5.9)*40.0 + t*(20.0 + 6.0*h11(fi)), top + 4.0);
    vec3 p = around(r, a, yy);
    float dd = raySeg(ro, rd, p, p + vec3(0.2, 1.4, 0.1), s);
    col += vec3(0.7, 0.82, 1.0)*(halo(dd, max(0.02, px))*0.9 + halo(dd, 0.12)*0.12)*form;
  }
  return col;
}

// ======== TATOOINE: golden blade in a vortex of sand and wind ========
vec3 ws_sand(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s, ta;
  vec3 col = ws_core(d, px, 0.36, vec3(1.0, 0.93, 0.72), vec3(1.0, 0.6, 0.2), 1.0 + ch + 0.3*st);
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (fx <= 0.0 || far > 8.0) return col;
  float wr = 1.0 + 2.0*(1.0 - form);
  // the sand vortex (marched)
  col += ws_puffs(ro, rd, 3.0, top, form, st, ch)*fx;
  // wind streaks whipping round
  for (int i = 0; i < 8; i++){
    float fi = float(i);
    float cyc = t*0.8 + h11(fi*4.3);
    float k = floor(cyc), f = fract(cyc);
    float r = (0.9 + 3.0*h11(fi*2.2 + k))*wr;
    float y = (0.03 + 0.9*h11(fi*1.7 + k*3.1))*top;
    float a = h11(fi*8.3 + k) + f*1.1;
    col += vec3(1.0, 0.9, 0.72)*ws_wind(ro, rd, r, a, y, 0.28, 0.8, px)*sin(3.1416*f)*(0.8 + 0.6*st)*form;
  }
  // glinting grains
  for (int i = 0; i < 24; i++){
    float fi = float(i);
    float r = (0.8 + 3.5*h11(fi*3.3))*wr;
    float a = h11(fi*7.7) + t*(0.5 + 0.3*h11(fi*1.1))/(0.4 + 0.25*r);
    float yy = mod(h11(fi*5.9)*(top + 2.0) + t*(0.8 + h11(fi*2.4)), top + 2.0) - 1.0;
    float dd = rayPt(ro, rd, around(r, a, yy), ta);
    col += vec3(1.0, 0.85, 0.6)*halo(dd, max(0.05, px))*1.5*(0.5 + 0.5*sin(t*9.0 + fi*2.0))*form;
  }
  return col;
}

// ======== CRAIT: white salt crust (solid pass) over red, rock shards, red dust ========
vec3 ws_shard(float fi, float form, out vec3 b0){
  float t = uTime;
  float r = (2.0 + 2.8*h11(fi*3.3))*(1.0 + 1.0*(1.0 - form));
  vec3 c = around(r, h11(fi*7.1) + t*0.04*(1.0 + h11(fi)), (0.08 + 0.84*h11(fi*1.9))*uLength + 0.4*sin(t*0.6 + fi));
  vec3 dir = normalize(vec3(sin(t*0.7 + fi), cos(t*0.5 + fi*2.0), sin(t*0.6 + fi*3.0)));
  float ln = 0.5 + 0.6*h11(fi*5.5);
  b0 = c - dir*ln;
  return c + dir*ln;
}
vec3 ws_rock(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s;
  float pul = 0.85 + 0.15*sin(t*2.0);
  vec3 col = ws_core(d, px, 0.42, vec3(1.0, 0.86, 0.84), vec3(1.0, 0.13, 0.1), (1.1 + 1.5*ch + 0.3*st)*pul);
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (fx <= 0.0 || far > 8.0) return col;
  // red salt dust rising off the cracks (marched)
  col += ws_puffs(ro, rd, 4.0, top, form, st, ch)*fx;
  // red light round the floating shards (the shards themselves are in the solid pass)
  for (int i = 0; i < 9; i++){
    float fi = float(i);
    vec3 b0;
    vec3 b1 = ws_shard(fi, form, b0);
    if (b1.y > top) continue;
    float dd = raySeg(ro, rd, b0, b1, s);
    col += vec3(1.0, 0.15, 0.1)*halo(dd, 0.45)*0.35*form;
  }
  return col;
}

// ======== MON CALA: a flowing column of water, spiralling streams, spray ========
vec3 ws_water(vec3 ro, vec3 rd, float px, float d, float top, float form, float ch, float st, float fx){
  float t = uTime, s, ta;
  vec3 col = ws_core(d, px, 0.3, vec3(0.85, 1.0, 1.0), vec3(0.15, 0.6, 0.95), 1.0 + ch + 0.3*st);
  // the flowing column (marched): water racing up, with bright caustic ridges
  col += ws_puffs(ro, rd, 5.0, top, form, st, ch)*fx;
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (fx <= 0.0 || far > 8.0) return col;
  // three streams of water spiralling up round the blade
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float best = 1e3, by = 0.0;
    float r = (1.45 + 0.2*sin(t*1.3 + fi*2.0))*(1.0 + 2.0*(1.0 - form))*(1.0 + 0.2*st);
    vec3 prev = around(r*0.6, fi/3.0 - t*0.35, 0.3);
    for (int j = 1; j <= 16; j++){
      float f = float(j)/16.0;
      float yy = 0.3 + f*(top - 0.8);
      vec3 q = around(r*(0.6 + 0.4*smoothstep(0.0, 0.15, f)), fi/3.0 + f*1.25 - t*0.35, yy);
      float dd = raySeg(ro, rd, prev, q, s);
      if (dd < best){ best = dd; by = (float(j) - 1.0 + s)/16.0; }
      prev = q;
    }
    float fl = 0.5 + 0.8*vn(vec2(by*30.0 - t*6.0, fi*5.0));
    col += (vec3(0.8, 1.0, 1.0)*halo(best, max(0.05, px))*1.2*fl + vec3(0.2, 0.7, 1.0)*halo(best, 0.2)*0.55 + vec3(0.1, 0.45, 0.9)*halo(best, 0.6)*0.15)*form*fx;
  }
  // spray thrown off the streams
  for (int i = 0; i < 30; i++){
    float fi = float(i);
    float f = fract(t*(0.7 + 0.4*h11(fi*2.9)) + h11(fi*5.1));
    float a0 = h11(fi*1.9) + floor(t*0.7 + h11(fi*5.1))*0.37;
    vec3 p = around(1.5 + 2.8*f*(1.0 + 0.5*st), a0 + 0.12*f, h11(fi*3.7 + floor(t*0.7))*top + 1.5*f);
    float dd = rayPt(ro, rd, p, ta);
    col += vec3(0.7, 0.95, 1.0)*(halo(dd, max(0.05, px))*1.5 + halo(dd, 0.25)*0.12)*(1.0 - f)*smoothstep(0.0, 0.1, f)*form*fx;
  }
  return col;
}

// ======== the change: the old climate's debris flung out ========
// piece i of the burst: where it is lt seconds after, its trailing end in b0
vec3 ws_piece(float fi, float lt, float top, float k, out vec3 b0){
  float a = h11(fi*3.7 + k);
  float up = (h11(fi*7.1 + k) - 0.4)*0.9;
  vec3 dir = normalize(vec3(sin(6.2832*a), up, cos(6.2832*a)));
  float v = 7.0 + 7.0*h11(fi*1.3 + k);
  vec3 st = around(0.5, a, (0.04 + 0.9*h11(fi*1.9 + k))*top);
  vec3 p = st + dir*v*(1.0 - exp(-lt*2.4))/2.4;
  float ln = k < 0.5 ? 0.9 : k < 1.5 ? 0.6 : k < 2.5 ? 1.2 : k < 3.5 ? 1.6 : k < 4.5 ? 0.45 : 0.8;
  b0 = p - dir*ln*(0.4 + 0.6*exp(-lt*1.5));
  return p;
}
vec3 ws_burst(vec3 ro, vec3 rd, float px, float d, float top, float kp, float kn, float lt){
  float s;
  vec3 gp = ws_glow(kp);
  vec3 col = vec3(0.0);
  // the flash
  col += mix(vec3(1.0), gp, 0.3)*(halo(d, 0.6)*2.2 + halo(d, 1.8)*0.45 + halo(d, 4.0)*0.12)*exp(-lt*7.0);
  // the blast: a shell of light bursting outward from the whole blade
  float Rb = 3.4*(1.0 - exp(-lt*5.0));
  col += mix(vec3(1.0), gp, 0.5)*halo(abs(d - Rb), 0.3 + 0.25*Rb)*exp(-lt*4.5)*smoothstep(0.0, 0.03, lt)*0.55;
  // the explosion itself: a billowing shell of the old climate's fire, frost, storm, sand,
  // dust or spray rolling out from the whole blade, white-hot inside, fading as it spreads
  float nb = floor(uTime/WS_P);
  // the explosion itself: a billowing shell of the old climate's fire, frost, storm, sand,
  // dust or spray rolling out from the whole blade, white-hot inside, as soft puffs that
  // each face the camera
  if (lt < 1.1){
    float Rx = 0.6 + 5.2*(1.0 - exp(-lt*3.2));
    float ta;
    for (int i = 0; i < 28; i++){
      float fi = float(i);
      float a = h11(fi*3.1 + nb);
      float yy = (0.02 + 0.98*h11(fi*5.7 + nb))*top;
      float up = (h11(fi*7.9 + nb) - 0.5)*Rx*0.5;
      vec3 c = around(Rx*(0.6 + 0.4*h11(fi*1.3 + nb)), a, yy + up);
      float dd = rayPt(ro, rd, c, ta);
      if (dd > 4.0) continue;
      float heat = clamp(1.0 - lt*1.6, 0.0, 1.0);
      float w = 0.5 + 0.9*lt + 0.4*h11(fi*2.2);
      col += mix(gp*0.8, vec3(1.0, 0.95, 0.9), heat*heat)*(halo(dd, w)*0.55 + halo(dd, w*2.2)*0.1)*exp(-lt*2.6)*smoothstep(0.0, 0.05, lt);
    }
  }
  // shock rings tearing out across the blade, one after another
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float lr = max(lt - fi*0.07, 0.0);
    float R = 0.5 + 6.0*(1.0 - exp(-lr*2.6));
    float fade = exp(-lr*2.6)*smoothstep(0.0, 0.03, lr)*(1.0 - smoothstep(0.5, 1.0, lr));
    float y = top*(0.18 + 0.22*fi + 0.12*h11(nb*3.1 + fi));
    // round 13 (Creyon): no rings round the blade; a flash of light where each shock leaves it
    float ta;
    float dr = rayPtRaw(ro, rd, vec3(0.0, y, 0.0), ta);
    col += (vec3(1.0)*halo(dr, 0.4 + 0.3*R*0.2)*1.0 + gp*halo(dr, 1.0 + 0.3*R)*0.35)*fade;
  }
  // the new climate condensing up the blade from the hilt
  float fr = -1.0 + (top + 3.0)*smoothstep(0.1, 0.7, lt);
  float sf;
  float dsf = raySegRaw(ro, rd, vec3(0.0, -0.6, 0.0), vec3(0.0, top, 0.0), sf);
  float yy = -0.6 + sf*(top + 0.6);
  col += mix(vec3(1.0), ws_glow(kn), 0.5)*halo(dsf, 1.0)*halo(abs(yy - fr), 1.2)*1.6*smoothstep(0.08, 0.15, lt)*(1.0 - smoothstep(0.6, 0.8, lt));
  // a flash of the new climate's colour as it condenses
  col += ws_glow(kn)*halo(d, 1.1)*smoothstep(0.1, 0.4, lt)*exp(-max(lt - 0.4, 0.0)*3.0)*0.35;
  // debris of the old climate flung out in every direction
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (far > 9.0) return col;
  float life = 1.0 - smoothstep(0.5, 1.6, lt);
  float w = kp < 0.5 ? 0.11 : kp < 1.5 ? 0.06 : kp < 2.5 ? 0.03 : kp < 3.5 ? 0.035 : kp < 4.5 ? 0.08 : 0.07;
  vec3 hot = kp < 0.5 ? vec3(1.0, 0.75, 0.35) : kp < 1.5 ? vec3(0.9, 0.97, 1.0) : kp < 2.5 ? vec3(0.85, 0.9, 1.0)
           : kp < 3.5 ? vec3(1.0, 0.85, 0.55) : kp < 4.5 ? vec3(1.0, 0.3, 0.2) : vec3(0.75, 0.95, 1.0);
  for (int i = 0; i < 30; i++){
    float fi = float(i);
    vec3 b0;
    vec3 b1 = ws_piece(fi, lt, top, kp, b0);
    float dd = raySeg(ro, rd, b0, b1, s);
    float tip = 0.4 + 0.6*s;
    col += (hot*halo(dd, max(w*1.7*tip, px))*2.0 + gp*halo(dd, 0.5)*0.55)*life;
  }
  return col;
}

// ======== the big show of each climate, Storm-blade size, all round the blade ========
vec3 ws_big(vec3 ro, vec3 rd, float px, float k, float top, float form, float st, float ch){
  float t = uTime, s, ta;
  vec3 col = vec3(0.0);
  float boost = form*(1.0 + 0.6*st + 0.8*ch);
  if (k < 0.5){
    // Mustafar: tall tongues of fire curling up off the blade
    for (int i = 0; i < 12; i++){
      float fi = float(i);
      float rate = 0.7 + 0.4*h11(fi*2.3);
      float f = fract(t*rate + h11(fi*5.1));
      float cy = floor(t*rate + h11(fi*5.1));
      float y0 = (0.05 + 0.8*h11(fi*3.7 + cy))*top;
      float a0 = h11(fi*6.3 + cy);
      float H = (3.5 + 3.0*h11(fi*1.9 + cy))*(0.4 + 0.6*sin(3.1416*min(f*1.4, 1.0)));
      vec3 prev = around(0.45, a0, y0);
      float best = 1e3, bu = 0.0;
      for (int j = 1; j <= 6; j++){
        float u = float(j)/6.0;
        vec3 q = around(0.45 + H*0.55*u + 0.4*sin(u*4.0 + t*6.0 + fi), a0 + 0.07*sin(u*5.0 - t*4.0 + fi), y0 + H*u*1.2);
        float dd = raySeg(ro, rd, prev, q, s);
        if (dd < best){ best = dd; bu = (float(j) - 1.0 + s)/6.0; }
        prev = q;
      }
      float w = mix(0.8, 0.12, bu)*(1.0 - 0.4*f);
      float life = sin(3.1416*f);
      col += (ws_fire(1.0 - bu*0.7)*halo(best, max(w, px))*2.6 + vec3(1.0, 0.3, 0.03)*halo(best, w*2.5)*0.6)*life;
    }
    // lava bursts: blobs of molten rock thrown out on arcs, glowing, trailing sparks
    for (int i = 0; i < 14; i++){
      float fi = float(i);
      float f = fract(t*0.55 + h11(fi*7.7));
      float cy = floor(t*0.55 + h11(fi*7.7));
      float y0 = (0.1 + 0.8*h11(fi*2.9 + cy))*top;
      vec3 dir = normalize(vec3(sin(6.2832*h11(fi*4.1 + cy)), 0.9 + 0.6*h11(fi*1.3 + cy), cos(6.2832*h11(fi*4.1 + cy))));
      float v = 6.0 + 4.0*h11(fi*5.5 + cy);
      float tt = f*1.2;
      vec3 p = vec3(0.0, y0, 0.0) + dir*v*tt + vec3(0.0, -6.0*tt*tt, 0.0);
      vec3 q = vec3(0.0, y0, 0.0) + dir*v*max(tt - 0.08, 0.0) + vec3(0.0, -6.0*max(tt - 0.08, 0.0)*max(tt - 0.08, 0.0), 0.0);
      float dd = raySeg(ro, rd, q, p, s);
      float dh = rayPt(ro, rd, p, ta);
      col += (vec3(1.0, 0.5, 0.1)*halo(dd, max(0.06, px))*1.2 + vec3(1.0, 0.75, 0.3)*halo(dh, 0.22)*2.0 + vec3(1.0, 0.3, 0.02)*halo(dh, 0.8)*0.4)*(1.0 - f)*smoothstep(0.0, 0.05, f);
    }
  } else if (k < 1.5){
    // Hoth: great ice spikes jutting out of the blade, glinting, and shards whirling round it
    for (int i = 0; i < 14; i++){
      float fi = float(i);
      float y0 = (0.05 + 0.88*h11(fi*2.7))*top;
      float a = h11(fi*6.1) + fi*0.37;
      float grow = smoothstep(0.0, 0.6, form - 0.04*fi*0.0);
      float ln = (2.6 + 2.6*h11(fi*1.3))*grow*(1.0 + 0.3*st);
      vec3 b0 = around(0.5, a, y0), b1 = around(0.5 + ln, a + 0.02, y0 + ln*0.55);
      float dd = raySeg(ro, rd, b0, b1, s);
      float w = max(0.24*(1.0 - s) + 0.03, px);
      float glint = pow(0.5 + 0.5*sin(t*2.0 + fi*1.7), 16.0);
      col += (vec3(0.9, 0.97, 1.0)*halo(dd, w)*1.5 + vec3(0.4, 0.75, 1.0)*halo(dd, w*3.5)*0.4 + vec3(1.0)*halo(dd, w)*glint*2.0*step(0.8, s))*form;
    }
    for (int i = 0; i < 20; i++){
      float fi = float(i);
      float r = 2.0 + 3.0*h11(fi*3.9);
      float a = h11(fi*8.7) + t*(0.25 + 0.2*h11(fi))/(0.5 + 0.2*r);
      float yy = (0.05 + 0.9*h11(fi*5.3))*top + 0.8*sin(t + fi);
      vec3 c = around(r, a, yy);
      vec3 dv = normalize(vec3(sin(t*2.0 + fi), cos(t*1.7 + fi*2.0), sin(t*1.3 + fi*3.0)))*0.35;
      float dd = raySeg(ro, rd, c - dv, c + dv, s);
      col += (vec3(0.9, 0.97, 1.0)*halo(dd, max(0.05, px))*1.6 + vec3(0.5, 0.8, 1.0)*halo(dd, 0.3)*0.3)*boost;
    }
    // frost bursts: rings of frost blasting out along the blade
    for (int i = 0; i < 2; i++){
      float fi = float(i);
      float f = fract(t*0.6 + fi*0.5);
      float yr = (0.25 + 0.5*h11(floor(t*0.6 + fi*0.5) + fi*3.0))*top;
      // round 13: frost spraying off the blade instead of rings round it
      vec3 fx_ = fxSparks(ro, rd, px, yr - 1.0, yr + 1.0, 8.0, 2.0 + 3.0*f, 1.5, fi*3.3, vec3(0.6, 0.85, 1.0));
      col += fx_*(1.0 - f)*form*0.8;
    }
  } else if (k < 2.5){
    // Kamino: big Storm-blade lightning loops leaping off the blade, struck again and again
    for (int i = 0; i < 5; i++){
      float fi = float(i);
      float kk = floor(t*(7.0 + 3.0*st) + fi*0.31);
      if (h11(kk*1.7 + fi) < 0.25) continue;
      float y0 = (0.05 + 0.75*h11(kk*2.3 + fi*3.1))*top;
      float dy = 3.0 + 6.0*h11(kk*3.9 + fi);
      float a = h11(kk*5.1 + fi);
      float reach = (3.8 + 2.2*h11(kk*0.7 + fi))*(1.0 + 0.2*st);
      vec3 A = around(0.35, a, y0), M = around(reach, a + 0.05*(h11(kk) - 0.5), y0 + dy*0.5), C = around(0.35, a + 0.1, y0 + dy);
      float bd = min(bolt3(ro, rd, A, M, kk + fi*11.0, 0.35, px), bolt3(ro, rd, M, C, kk + fi*11.0 + 0.5, 0.35, px));
      float fade = exp(-fract(t*(7.0 + 3.0*st) + fi*0.31)*2.5);
      col += (boltLight(bd, vec3(0.45, 0.65, 1.0), px)*2.2 + vec3(0.4, 0.6, 1.0)*halo(bd, 1.5)*0.4)*fade*form;
    }
    // bolts leaping from the tip into the sky
    for (int i = 0; i < 2; i++){
      float fi = float(i);
      float kk = floor(t*5.0 + fi*0.5);
      if (h11(kk*3.3 + fi) < 0.45) continue;
      vec3 A = vec3(0.0, top, 0.0);
      vec3 C = A + around(2.0 + 2.0*h11(kk + fi), h11(kk*2.1 + fi), 3.5 + 2.0*h11(kk*4.4));
      float bd = bolt3(ro, rd, A, C, kk + fi*7.0, 0.4, px);
      col += boltLight(bd, vec3(0.5, 0.7, 1.0), px)*1.4*exp(-fract(t*5.0 + fi*0.5)*3.0)*form;
    }
    // sea spray flung off the blade
    for (int i = 0; i < 16; i++){
      float fi = float(i);
      float f = fract(t*0.8 + h11(fi*3.1));
      float y0 = (0.05 + 0.9*h11(fi*6.3 + floor(t*0.8 + h11(fi*3.1))))*top;
      vec3 p = around(0.5 + 4.0*f, h11(fi*9.1) + 0.1*f, y0 + 1.5*f - 2.5*f*f);
      float dd = rayPt(ro, rd, p, ta);
      col += vec3(0.7, 0.85, 1.0)*(halo(dd, max(0.05, px))*1.2 + halo(dd, 0.25)*0.2)*(1.0 - f)*form;
    }
  } else if (k > 3.5 && k < 4.5){
    // Crait: red energy cracking out from the blade to the floating boulders
    for (int i = 0; i < 9; i++){
      float fi = float(i);
      vec3 b0;
      vec3 b1 = ws_shard(fi, form, b0);
      if (b1.y > top) continue;
      float kk = floor(t*4.0 + fi*0.37);
      if (h11(kk*1.9 + fi) < 0.55) continue;
      vec3 c = (b0 + b1)*0.5;
      vec3 A = around(0.45, atan(c.x, c.z)/6.2832, c.y);
      float bd = bolt3(ro, rd, A, c, kk + fi*5.0, 0.35, px);
      col += (boltLight(bd, vec3(1.0, 0.12, 0.08), px)*1.2)*exp(-fract(t*4.0 + fi*0.37)*3.0)*form;
    }
    // red salt dust streaming off the boulders and blade
    for (int i = 0; i < 18; i++){
      float fi = float(i);
      float f = fract(t*0.5 + h11(fi*2.3));
      float y0 = (0.05 + 0.9*h11(fi*7.1 + floor(t*0.5 + h11(fi*2.3))))*top;
      vec3 p = around(0.6 + 4.2*f, h11(fi*4.9) + 0.25*f, y0 + 1.0*f);
      vec3 q = around(0.6 + 4.2*max(f - 0.06, 0.0), h11(fi*4.9) + 0.25*max(f - 0.06, 0.0), y0 + 1.0*max(f - 0.06, 0.0));
      float dd = raySeg(ro, rd, q, p, s);
      col += vec3(1.0, 0.2, 0.12)*(halo(dd, max(0.06, px))*1.1 + halo(dd, 0.35)*0.2)*sin(3.1416*f)*form;
    }
  }
  return col;
}

vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float top = ws_top(), lt = ws_lt(), k = ws_cur(), kp = ws_prev();
  float form = ws_form(), ch = ws_charge(), boom = ws_boom();
  float st = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0), fx = uFx;
  float s;
  float d = raySegRaw(ro, rd, vec3(0.0, -0.6, 0.0), vec3(0.0, top, 0.0), s);
  vec3 col = vec3(0.0);
  if (k < 0.5) col = ws_lava(ro, rd, px, d, top, form, ch, st, fx);
  else if (k < 1.5) col = ws_ice(ro, rd, px, d, top, form, ch, st, fx);
  else if (k < 2.5) col = ws_storm(ro, rd, px, d, top, form, ch, st, fx);
  else if (k < 3.5) col = ws_sand(ro, rd, px, d, top, form, ch, st, fx);
  else if (k < 4.5) col = ws_rock(ro, rd, px, d, top, form, ch, st, fx);
  else col = ws_water(ro, rd, px, d, top, form, ch, st, fx);
  col += ws_big(ro, rd, px, k, top, form, st, ch)*fx;
  // the straining climate trembles with light just before it bursts
  col += mix(vec3(1.0), ws_glow(k), 0.5)*halo(d, 0.9)*ch*ch*(0.6 + 0.4*sin(uTime*40.0))*0.8;
  if (boom > 0.5 && lt < 1.6) col += ws_burst(ro, rd, px, d, top, kp, k, lt)*fx;
  // clash: the ruling climate flashes
  if (cl > 0.01){
    float ta;
    float dc = rayPtRaw(ro, rd, vec3(0.0, min(uLength*0.6, top), 0.0), ta);
    col += (vec3(1.0)*halo(dc, 1.0)*2.5 + ws_glow(k)*halo(dc, 2.8)*1.2)*cl*cl;
    col += ws_glow(k)*halo(ws_ring(ro, rd, min(uLength*0.6, top), 0.5 + 5.0*(1.0 - cl)), 0.25)*cl*1.2;
  }
  return (1.0 - exp(-col*1.1))*1.05;
}

// ======== the solid parts: crusts (Mustafar basalt, Crait salt), chunks and shards ========
vec4 sig_vol_solid(vec3 ro, vec3 rd, float px){
  float top = ws_top(), lt = ws_lt(), k = ws_cur(), kp = ws_prev();
  float form = ws_form(), ch = ws_charge(), boom = ws_boom();
  float s, ta, tc = 1e5;
  vec4 o = vec4(0.0);

  float oldCrust = kp < 0.5 ? 1.0 : (kp > 3.5 && kp < 4.5 ? 1.0 : 0.0);
  if (uFx <= 0.0) return o;
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (far > 9.0) return o;
  // chunks of the old crust flung out (basalt or salt)
  if (boom > 0.5 && oldCrust > 0.5 && lt < 1.6){
    float life = 1.0 - smoothstep(0.7, 1.6, lt);
    for (int i = 0; i < 14; i++){
      float fi = float(i);
      vec3 b0;
      vec3 b1 = ws_piece(fi, lt, top, kp, b0);
      float dd = raySeg(ro, rd, b0, b1, s);
      float w = (0.09 + 0.06*h11(fi*2.3))*(0.25 + 0.75*sqrt(sin(3.1416*s)));
      float al = fillD(dd - w, px)*life;
      if (al <= 0.0) continue;
      float tmid = rayPt(ro, rd, (b0 + b1)*0.5, ta);
      float rim = smoothstep(w*0.3, w, dd);
      vec3 c = kp < 0.5 ? vec3(0.08, 0.05, 0.04)*(1.0 - 0.5*rim) + vec3(1.0, 0.35, 0.05)*rim*0.8
                        : vec3(0.88, 0.86, 0.84)*(0.95 - 0.35*rim) + vec3(1.0, 0.12, 0.1)*rim*0.5;
      o = vec4(mix(o.rgb, c, al), max(o.a, al));
    }
  }
  // Crait: salt shards drifting round the blade
  if (k > 3.5 && k < 4.5){
    for (int i = 0; i < 9; i++){
      float fi = float(i);
      vec3 b0;
      vec3 b1 = ws_shard(fi, form, b0);
      if (b1.y > top) continue;
      float dd = raySeg(ro, rd, b0, b1, s);
      float w = (0.32 + 0.22*h11(fi*4.1))*(0.2 + 0.8*sqrt(sin(3.1416*s)));
      float al = fillD(dd - w, px)*form;
      if (al <= 0.0) continue;
      float tmid = rayPt(ro, rd, (b0 + b1)*0.5, ta);
      float rim = smoothstep(w*0.2, w, dd);
      vec3 c = vec3(0.9, 0.88, 0.86)*(1.0 - 0.45*rim) + vec3(1.0, 0.12, 0.1)*rim*0.45;
      o = vec4(mix(o.rgb, c, al), max(o.a, al));
    }
  }
  return o;
}

// the blade: Sol's glow and core from the file, taking the ruling climate's colour (and
// the next one's as it condenses up the blade after a change)
vec3 ws_tint(vec3 g, vec2 b){
  float k = ws_cur(), lt = ws_lt();
  float l = max(g.r, max(g.g, g.b));
  vec3 cn = ws_glow(k);
  float fr = ws_boom() > 0.5 ? -1.0 + (uLength + 3.0)*smoothstep(0.1, 0.7, lt) : 1e5;
  vec3 c = b.y < fr ? cn : ws_glow(ws_prev());
  return l*c/max(max(c.r, max(c.g, c.b)), 0.01);
}
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float ch = ws_charge();
  return ws_tint(g, b)*(1.0 + ch*ch*(0.6 + 0.4*sin(uTime*40.0)) + 0.3*uSwing);
}
vec3 sig_core(vec3 c, vec2 b, float px){
  vec3 t = ws_tint(c, b);
  return mix(c, t, 0.35);
}
// the solid sheaths, drawn over Sol's glow as his solid blades are: Mustafar's cracked
// basalt crust, Hoth's ice crystal, Crait's white salt crust over the red
vec4 sig_solid(vec4 o, vec2 b, float px){
  float k = ws_cur(), top = ws_top(), form = ws_form(), ch = ws_charge();
  float yc = -0.3 + (top + 0.3)*form;
  if (k > 0.5 && k < 1.5){
    // ice: a pale faceted sheath, see-through, its edges bright, frost on it
    float A = 0.62;
    float cov = fillD(abs(b.x) - A, px)*smoothstep(0.1, 0.8, b.y)*(1.0 - smoothstep(yc - 2.5, yc, b.y));
    if (cov <= 0.0) return o;
    float u = b.x/A;
    float frost = vn(vec2(u*9.0, b.y*4.0))*vn(vec2(u*3.0 + 5.0, b.y*1.3));
    float edge = halo(1.0 - abs(u), 0.06) + halo(abs(u) - 0.5, 0.03 + px)*0.6;
    vec3 c = vec3(0.5, 0.8, 1.0)*(0.15 + 0.5*frost) + vec3(0.9, 0.97, 1.0)*edge*0.9;
    c += vec3(1.0)*halo(abs(vn(vec2(u*2.0 + 9.0, b.y*0.6)) - 0.5), 0.02)*ch*1.5;
    float al = cov*(0.35 + 0.4*frost + 0.3*edge);
    return vec4(mix(o.rgb, c, al), max(o.a, al));
  }
  if (k < 0.5 || (k > 3.5 && k < 4.5)){
    float Rc = 0.6;
    float y1 = min(top - 0.9, form*(top + 1.0));
    float cov = fillD(abs(b.x) - Rc, px)*smoothstep(0.1, 0.7, b.y)*(1.0 - smoothstep(y1 - 0.4, y1 + 0.6, b.y));
    if (cov <= 0.0) return o;
    float u = clamp(b.x/Rc, -1.0, 1.0);
    vec2 cell;
    float e = k < 0.5 ? voro(vec2(u*2.5, b.y*1.0 + uTime*0.22), cell) : voro(vec2(u*2.5, b.y*1.2), cell);
    float w = (k < 0.5 ? 0.13 + 0.05*sin(uTime*2.0 + cell.x*3.0 + cell.y) : 0.04 + 0.03*sin(uTime*1.5 + cell.x*2.0 + cell.y*1.3)) + ch*0.12;
    float al = cov*smoothstep(w, w + 0.06, e);
    float dif = clamp(0.55 - 0.45*u + 0.2*sqrt(1.0 - u*u), 0.0, 1.0);
    float nz = vn(vec2(u*12.0, b.y*6.0));
    float glow = exp(-max(e - w, 0.0)/(k < 0.5 ? 0.07 : 0.09));
    vec3 c;
    if (k < 0.5) c = vec3(0.09, 0.055, 0.04)*(0.6 + 0.7*nz)*(0.45 + 0.55*dif) + vec3(1.0, 0.32, 0.04)*glow*1.2;
    else c = vec3(1.0, 0.98, 0.96)*(0.75 + 0.25*nz)*(0.55 + 0.6*dif) + vec3(1.0, 0.1, 0.08)*glow;
    return vec4(mix(o.rgb, c, al), max(o.a, al));
  }
  return o;
}

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxSparks(ro, rd, px, y0, y1, 12.0, 6.0, 0.8, 6.4, vec3(0.95, 0.75, 0.6))*part(1.0);
  return col + fx*uFx;
}
