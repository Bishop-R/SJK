// Exogorth: the blade is the space slug's living gullet: a real, round, ribbed and rubbery throat
// with a molten amber core inside. Slow swallowing waves bulge up it and the light glows hot through
// the stretched hide between its rings. At the emitter it opens as a maw: a fleshy lip ringed with
// two rows of pale fangs that the blade pushes out of, glowing strands of saliva sagging between the
// fangs and the throat. All round it hangs the dark of an asteroid cave: dust and grit swirl in a
// slow 3D whirlpool and are sucked into the throat, lumps of rock tumble in and are crushed in
// sparks. A swing makes the throat gulp; a clash is the slug's roar: a shockwave rolls out, a burst
// of hot breath and a spray of rocks blow outward and the fangs flare open; on ignition the gullet
// swells up out of the maw.
// @part Asteroid cave
// @part Crushed rock sparks
// @motes Dust
// @wisps Hot breath
// @room 10
// @solid 1
// @palette {"corona.rim_cool":[0.55,0.20,0.04],"corona.rim_hot":[1.0,0.55,0.18],"corona.inner":[1.0,0.88,0.65],"core.white":[1.0,0.95,0.85],"core.fringe_cool":[1.0,0.45,0.12],"core.fringe_hot":[1.0,0.75,0.40],"light.color":[1.0,0.55,0.20],"trail":[1.0,0.55,0.20]}

float ex_top(){ float g = clamp(uAge/0.5, 0.0, 1.0); return (uLength + 0.2)*(1.0 - (1.0 - g)*(1.0 - g)); }
// the swallowing waves: how much the throat bulges at y
float ex_bulge(float y){
  float t = uTime;
  float w = pow(0.5 + 0.5*sin(y*0.32 - t*2.2), 4.0) + 0.6*pow(0.5 + 0.5*sin(y*0.21 - t*1.4 + 2.0), 6.0);
  return w*(1.0 + 1.2*uSwing);
}
// the rings of the throat: 1 on a ridge, 0 between
float ex_ring(float y){ return 0.5 + 0.5*cos(y*3.2); }
float ex_w(float y){
  float L = uLength, top = ex_top();
  float w = (0.46 + 0.2*ex_bulge(y))*(1.0 + 0.09*ex_ring(y));
  w *= 1.0 - 0.75*smoothstep(top - 1.4, top + 0.1, y);
  return w;
}
// how far the maw is open: breathing slowly, flaring on a clash
float ex_gape(){ return 0.5 + 0.3*sin(uTime*0.7) + 0.9*uClash + 0.3*uSwing; }
// the gullet, drawn over Sol's glow and core as his solid blades are: a ribbed, rubbery
// throat facing the camera, lit from one side, the molten light glowing through its hide
// where the swallowing waves stretch it thin
vec4 sig_solid(vec4 o, vec2 b, float px){
  float top = ex_top();
  if (top < 0.3) return o;
  float y = b.y;
  float w = ex_w(y);
  float cov = fillD(abs(b.x) - w, px)*smoothstep(0.0, 0.3, y)*(1.0 - smoothstep(top - 0.1, top + 0.3, y));
  if (cov <= 0.0) return o;
  float u = clamp(b.x/max(w, 0.01), -1.0, 1.0);
  float dif = clamp(0.55 - 0.45*u + 0.2*sqrt(1.0 - u*u), 0.0, 1.0);
  float ndv = sqrt(1.0 - u*u);
  float ring = ex_ring(y);
  float bu = ex_bulge(y);
  float thin = clamp(bu*0.8 + 0.15, 0.0, 1.0)*(1.0 - ring*0.7);
  float nz = fbm3(vec2(u*1.5 + y*0.3, y*1.2));
  vec3 hide = vec3(0.24, 0.21, 0.19)*(0.5 + 0.6*nz)*(0.35 + 0.65*dif)*(0.6 + 0.4*ring);
  vec3 glowT = mix(vec3(1.0, 0.35, 0.08), vec3(1.0, 0.85, 0.5), thin)*thin*(0.35 + 0.75*ndv);
  float vein = halo(abs(vn(vec2(u*2.5, y*0.8)) - 0.5), 0.035);
  vec3 rgb = mix(hide, glowT*1.3, smoothstep(0.15, 0.8, thin)) + vec3(1.0, 0.4, 0.1)*vein*thin*0.7;
  rgb += vec3(1.0, 0.45, 0.12)*(1.0 - ring)*0.25*(0.5 + bu)*ndv;
  rgb += vec3(0.9, 0.8, 0.7)*pow(halo(u + 0.4, 0.25), 2.0)*0.3*(0.4 + 0.6*ring);
  // the gulp: a hot pulse rising up the throat now and then
  float gy = mod(uTime*7.0, uLength + 30.0) - 5.0;
  rgb += vec3(1.0, 0.7, 0.35)*halo(y - gy, 1.5)*ndv*0.8;
  return vec4(mix(o.rgb, rgb, cov), max(o.a, cov));
}
// a lump of rock: how much it covers the ray (th its depth, sh its face shade)
float ex_rock(vec3 ro, vec3 rd, vec3 c, float sz, float spin, float seed, float px, out float th, out float sh){
  float d = rayPt(ro, rd, c, th);
  sh = 0.0;
  if (d > sz*1.6 + px) return 0.0;
  vec3 o = ro + rd*th - c;
  vec2 a = rot(spin + seed*4.0)*o.xy; o = vec3(a, o.z);
  vec2 b = rot(spin*0.6 + seed*2.0)*o.xz; o = vec3(b.x, o.y, b.y);
  float k = max(max(abs(o.x) + abs(o.y)*0.5, abs(o.y)*1.1 + abs(o.z)*0.4), abs(o.z) + abs(o.x)*0.4) - sz + 0.25*sz*vn(o.xy*5.0/sz + seed);
  sh = 0.45 + 0.55*h21(sign(o.xy)*1.3 + vec2(seed, sign(o.z)));
  return clamp(0.5 - k/max(px, 1e-3), 0.0, 1.0);
}
// rock i being dragged in round the blade
vec3 ex_rockPos(float fi, out float f, out float sz){
  float L = uLength, t = uTime;
  float rate = 0.10 + 0.05*h11(fi*2.3);
  f = fract(t*rate + h11(fi*6.1));
  float cyc = floor(t*rate + h11(fi*6.1));
  float a0 = h11(fi*3.7 + cyc*1.9);
  float y = (0.12 + 0.75*h11(fi*9.3 + cyc))*L;
  float r = 0.6 + 6.0*(1.0 - f)*(1.0 - f);
  sz = (0.35 + 0.45*h11(fi*5.0 + cyc))*(1.0 - smoothstep(0.85, 1.0, f));
  return around(r, a0 + 0.35*f, y + 2.0*(1.0 - f));
}
// the maw's fangs: two rows of pale curved fangs round the emitter, each a tapered piece
// facing the camera; f returns how much of the ray it covers, th its depth, k the shade
float ex_fangs(vec3 ro, vec3 rd, float px, out float th, out float k){
  float g = ex_gape();
  float best = 0.0; th = 1e5; k = 0.0;
  for (int i = 0; i < 18; i++){
    float fi = float(i);
    float inner = fi >= 11.0 ? 1.0 : 0.0;
    float a = inner > 0.5 ? (fi - 11.0 + 0.5)/7.0 : fi/11.0;
    float r0 = inner > 0.5 ? 0.62 : 0.82;
    float len = inner > 0.5 ? 0.7 + 0.3*h11(fi*7.0) : 1.1 + 0.45*h11(fi*3.0);
    float lean = inner > 0.5 ? -0.1 + 0.25*g : 0.1 + 0.4*g;
    vec3 p0 = around(r0, a, 0.2);
    vec3 pm = around(r0 + lean*0.5 - 0.12*len, a, 0.2 + len*0.55);
    vec3 p1 = around(r0 + lean, a, 0.2 + len);
    // 31/10/2026: a fang covers only what lies within its thickness (0.2 at most) and half a
    // pixel of its two pieces, all within reach of its middle point: a ray further off skips it
    float tm;
    if (rayPtRaw(ro, rd, pm, tm) - max(length(p0 - pm), length(p1 - pm)) > 0.25 + px) continue;
    float s0, s1;
    float d0 = raySeg(ro, rd, p0, pm, s0);
    float d1 = raySeg(ro, rd, pm, p1, s1);
    float h = d0 < d1 ? s0*0.55 : 0.55 + s1*0.45;
    float d = min(d0, d1) - mix(0.2, 0.015, pow(h, 0.7));
    float cv = fillD(d, px);
    if (cv > 0.0){
      float tt;
      rayPt(ro, rd, d0 < d1 ? mix(p0, pm, s0) : mix(pm, p1, s1), tt);
      if (tt < th){ th = tt; best = cv; k = h; }
    }
  }
  return best;
}
vec4 sig_vol_solid(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime;
  vec4 col = vec4(0.0);
  float top = ex_top();
  float bestT = 1e5;
  // ---- the maw's fangs ----
  if (top > 0.2){
    float th, k;
    float cv = ex_fangs(ro, rd, px, th, k);
    if (cv > 0.0){
      vec3 rgb = vec3(0.80, 0.74, 0.62)*(0.55 + 0.35*(1.0 - k))*(0.8 + 0.2*vn(vec2(k*9.0, th))) + vec3(1.0, 0.5, 0.15)*(1.0 - k)*0.45;
      col = vec4(rgb, cv*smoothstep(0.2, 0.6, top));
      bestT = th;
    }
  }
  if (uFx < 0.01) return col;
  // ---- rocks dragged in and crushed ----
  for (int i = 0; i < 12; i++){
    float fi = float(i);
    float f, sz;
    vec3 c = ex_rockPos(fi, f, sz);
    if (c.y > top) continue;
    float th, sh;
    float cv = ex_rock(ro, rd, c, sz, t*(0.6 + h11(fi)), fi, px, th, sh);
    if (cv > 0.0 && th < bestT){
      bestT = th;
      vec3 rgb = vec3(0.22, 0.19, 0.17)*sh + vec3(1.0, 0.5, 0.15)*(0.15 + 0.6*smoothstep(0.4, 1.0, f))*(1.0 - sh);
      col = vec4(mix(col.rgb, rgb, cv), max(col.a, cv));
    }
  }
  // ---- the roar: a spray of rocks blown out from where it was struck ----
  if (uClash > 0.01){
    float a = 1.0 - uClash;
    for (int i = 0; i < 12; i++){
      float fi = float(i);
      vec3 dir = normalize(vec3(sin(fi*2.39), 0.5*(h11(fi*3.0) - 0.4), cos(fi*2.39)));
      vec3 c = vec3(0.0, L*0.6, 0.0) + dir*(0.9 + 7.0*a) + vec3(0.0, -2.0*a*a, 0.0);
      float th, sh;
      float cv = ex_rock(ro, rd, c, (0.25 + 0.15*h11(fi*7.0))*(1.0 - smoothstep(0.6, 1.0, a)), a*8.0, fi + 20.0, px, th, sh);
      if (cv > 0.0 && th < bestT){
        bestT = th;
        col = vec4(mix(col.rgb, vec3(0.25, 0.22, 0.2)*sh + vec3(1.0, 0.55, 0.2)*(1.0 - sh)*uClash, cv), max(col.a, cv));
      }
    }
  }
  return col;
}
// a strand of saliva i: point at u (0 on a fang .. 1 on the throat), sagging and swaying
vec3 ex_strand(float fi, float u){
  float t = uTime;
  float a = fi/6.0 + 0.04*sin(t*0.9 + fi);
  float g = ex_gape();
  vec3 p0 = around(0.9 + 0.3*g, a, 1.1 + 0.3*h11(fi*3.0));
  vec3 p1 = around(0.5, a + 0.08, 2.2 + 1.8*h11(fi*5.0));
  vec3 p = mix(p0, p1, u);
  p.y -= sin(u*3.1416)*(0.5 + 0.25*sin(t*1.3 + fi*2.0));
  p += around(0.08*sin(u*3.1416)*sin(t*2.0 + fi), a + 0.25, 0.0);
  return p;
}
vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime;
  float top = ex_top();
  vec3 amb = vec3(1.0, 0.55, 0.2);
  float s, tq;
  // The blade's own light is Sol's glow and core from the file (sig_glow, sig_core); here
  // only the glow in the maw, and the effects, each a piece placed round it in 3D.
  vec3 light = vec3(1.0, 0.6, 0.25)*halo(rayPtRaw(ro, rd, vec3(0.0, 0.5, 0.0), tq), 0.9)*0.5*step(0.2, top);
  if (uFx < 0.01 || top < 1.0) return light;
  vec3 fx = vec3(0.0);
  float d = raySegRaw(ro, rd, vec3(0.0, 0.0, 0.0), vec3(0.0, top, 0.0), s);
  float ys = s*top;
  // the cave's dust: a slow whirlpool of soft grey-amber puffs drawn in round the blade
  for (int i = 0; i < 18; i++){
    float fi = float(i);
    float f = fract(t*(0.06 + 0.03*h11(fi*2.7)) + h11(fi*8.3));
    float r = 1.2 + 4.0*(1.0 - f);
    vec3 p = around(r, h11(fi*1.7) + t*0.5/(r + 0.8) + 0.6*f, (0.06 + 0.88*h11(fi*4.9))*L);
    float dd = rayPt(ro, rd, p, tq);
    if (dd > 3.5) continue;
    fx += mix(vec3(0.30, 0.25, 0.20), vec3(1.0, 0.55, 0.25), exp(-r/1.8))*(halo(dd, 0.9 + 0.5*h11(fi*3.1))*0.2 + halo(dd, 2.0)*0.04)*sin(3.1416*f);
  }
  // grit and specks spiralling in on long streaks, crushed into sparks at the throat
  for (int i = 0; i < 32; i++){
    float fi = float(i);
    float rate = 0.15 + 0.1*h11(fi*3.1);
    float f = fract(t*rate + h11(fi*5.7));
    float cyc = floor(t*rate + h11(fi*5.7));
    float y = (0.08 + 0.85*h11(fi*2.9 + cyc))*L;
    if (y > top) continue;
    float R = 0.6 + 6.5*(1.0 - f)*(1.0 - f);
    float a = h11(fi*1.3 + cyc*2.7) + 0.6*f;
    vec3 p = around(R, a, y + 1.5*(1.0 - f)*sin(fi));
    vec3 q = around(R + 0.5*(1.0 - f) + 0.05, a - 0.03, y + 1.5*(1.0 - f)*sin(fi));
    float dd = raySeg(ro, rd, p, q, s);
    fx += vec3(1.0, 0.75, 0.45)*halo(dd, max(0.05, px))*(0.9 + 1.2*h11(fi*7.7))*smoothstep(0.0, 0.3, f)*s;
    // 31/10/2026: its flash at the throat only shows in the last tenth of its way in, so it is
    // only worked out then (the same picture)
    if (f > 0.9) fx += vec3(1.0, 0.7, 0.3)*halo(rayPt(ro, rd, around(ex_w(y) + 0.1, a, y), tq), 0.3)*smoothstep(0.9, 1.0, f)*1.4;
  }
  // crushed rock: sparks spraying out of the throat where each rock is swallowed
  for (int i = 0; i < 12; i++){
    float fi = float(i);
    float f, sz;
    vec3 c = ex_rockPos(fi, f, sz);
    if (c.y > top || f < 0.86) continue;
    float k = (f - 0.86)/0.14;
    for (int j = 0; j < 5; j++){
      float fj = float(j);
      vec3 dir = normalize(vec3(sin(fj*2.1 + fi), 0.6*(h11(fj + fi*3.0) - 0.3), cos(fj*2.1 + fi)));
      vec3 c0 = around(ex_w(c.y), atan(c.x, c.z)/6.2832, c.y);
      float dd = raySeg(ro, rd, c0 + dir*2.5*k, c0 + dir*(2.5*k + 0.5), s);
      fx += vec3(1.0, 0.8, 0.45)*halo(dd, max(0.04, px))*1.6*(1.0 - k);
    }
  }
  // hot breath: rings of vapour puffing out of the maw with the gape, rising and widening
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float f = fract(t*0.35 + fi/3.0);
    float R = 1.0 + 2.8*f;
    float yb = 1.2 + 3.0*f;
    vec3 h = rayAt(ro, rd, yb);
    if (h.z > 0.0) fx += amb*halo(hideD(abs(length(h.xy) - R), behindBlade(ro, rd, h.z)), 0.15 + 0.5*f)*(1.0 - f)*0.35*ex_gape();
  }
  // acid: glowing drops sliding down the outside of the gullet and falling
  for (int i = 0; i < 8; i++){
    float fi = float(i);
    float f = fract(t*(0.3 + 0.15*h11(fi*2.2)) + h11(fi*4.4));
    float k = floor(t*(0.3 + 0.15*h11(fi*2.2)) + h11(fi*4.4));
    float y0 = (0.2 + 0.7*h11(fi*6.6 + k))*L;
    float an = h11(fi*9.9 + k);
    vec3 p = around(ex_w(y0) + 0.15 + 0.5*f, an, y0 - 6.0*f*f);
    float dd = raySeg(ro, rd, p, p + vec3(0.0, 0.2 + 0.9*f, 0.0), s);
    fx += (vec3(1.0, 0.85, 0.4)*halo(dd, max(0.05, px))*1.3 + amb*halo(dd, 0.3)*0.3)*(1.0 - f)*step(y0, top);
  }
  // glowing saliva strands sagging between the fangs and the throat, a drop gathering on each
  if (top > 3.0){
    for (int i = 0; i < 6; i++){
      float fi = float(i);
      // 31/10/2026: a strand keeps within half its chord, its sag (0.75) and sway (0.08) of its
      // chord's middle, its drop 0.12 under it; its widest glow is 0.2 (and a pixel's): a ray
      // further off than three of them skips its four pieces and its drop
      vec3 e0 = ex_strand(fi, 0.0), e1 = ex_strand(fi, 1.0);
      float tm;
      if (rayPtRaw(ro, rd, 0.5*(e0 + e1), tm) - 0.5*length(e1 - e0) - 0.95 > 0.6 + 3.0*px) continue;
      vec3 prev = e0;
      float best = 1e3;
      for (int k = 1; k <= 4; k++){
        vec3 nx = ex_strand(fi, float(k)/4.0);
        best = min(best, raySeg(ro, rd, prev, nx, s));
        prev = nx;
      }
      float drop = rayPt(ro, rd, ex_strand(fi, 0.5) - vec3(0.0, 0.12, 0.0), tq);
      fx += vec3(1.0, 0.7, 0.35)*halo(best, max(0.03, px*0.7))*0.8 + amb*halo(best, 0.2)*0.45 + vec3(1.0, 0.85, 0.6)*halo(drop, max(0.07, px))*0.9;
    }
  }
  // the roar: a shockwave rolling out round the blade, a blast of light, a pulse up the gullet
  if (uClash > 0.01){
    float a = 1.0 - uClash;
    vec3 h = rayAt(ro, rd, L*0.6);
    if (h.z > 0.0) fx += amb*halo(hideD(abs(length(h.xy) - 1.0 - 7.0*a), behindBlade(ro, rd, h.z)), 0.22 + 0.3*a)*1.6*uClash;
    float dc = rayPtRaw(ro, rd, vec3(0.0, L*0.6, 0.0), tq);
    fx += amb*halo(abs(dc - 1.0 - 5.0*a), 0.3 + 0.4*a)*0.7*uClash + amb*halo(dc, 1.5)*pow(uClash, 2.0)*1.2;
    fx += amb*halo(max(d - ex_w(ys), 0.0), 1.2)*uClash*0.6;
  }
  return light + fx;
}
// the blade: Sol's glow and core from the file, swelling with the swallowing waves
vec3 sig_glow(vec3 g, vec2 b, float r, float px){ return g*(0.8 + 0.5*ex_bulge(b.y) + 0.6*uClash); }
vec3 sig_core(vec3 c, vec2 b, float px){ return c; }

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxSparks(ro, rd, px, y0, y1, 18.0, 6.5, 0.9, 5.5, vec3(1.0, 0.55, 0.2))*part(1.0);
  return col + fx*uFx;
}
