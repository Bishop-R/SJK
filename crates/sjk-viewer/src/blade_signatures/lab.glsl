
// ---- signature helpers ----
float h11(float p){ p=fract(p*0.1031); p*=p+33.33; p*=p+p; return fract(p); }
float h21(vec2 p){ vec3 q=fract(vec3(p.xyx)*0.1031); q+=dot(q,q.yzx+33.33); return fract((q.x+q.y)*q.z); }
vec2 h22(vec2 p){ vec3 q=fract(vec3(p.xyx)*vec3(0.1031,0.1030,0.0973)); q+=dot(q,q.yzx+33.33); return fract((q.xx+q.yz)*q.zy); }
float vn(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
  return mix(mix(h21(i),h21(i+vec2(1.0,0.0)),f.x),mix(h21(i+vec2(0.0,1.0)),h21(i+vec2(1.0)),f.x),f.y); }
float fbm(vec2 p){ float s=0.0, a=0.5; for(int i=0;i<5;i++){ s+=a*vn(p); p=p*2.03+vec2(1.7,9.2); a*=0.5; } return s; }
float fbm3(vec2 p){ float s=0.0, a=0.5; for(int i=0;i<3;i++){ s+=a*vn(p); p=p*2.03+vec2(1.7,9.2); a*=0.5; } return s/0.875; }
vec3 hsv(float h, float s, float v){ vec3 p=abs(fract(vec3(h)+vec3(0.0,2.0/3.0,1.0/3.0))*6.0-3.0); return v*mix(vec3(1.0),clamp(p-1.0,0.0,1.0),s); }
mat2 rot(float a){ float c=cos(a), s=sin(a); return mat2(c,-s,s,c); }
float sdSeg(vec2 p, vec2 a, vec2 b){ vec2 pa=p-a, ba=b-a; float h=clamp(dot(pa,ba)/dot(ba,ba),0.0,1.0); return length(pa-ba*h); }
float sdBox(vec2 p, vec2 b){ vec2 d=abs(p)-b; return length(max(d,0.0))+min(max(d.x,d.y),0.0); }
float sdEll(vec2 p, vec2 r){ float k=length(p/r); return (k-1.0)*min(r.x,r.y); }
float fillD(float d, float px){ return clamp(0.5-d/max(px,1e-4),0.0,1.0); }
float halo(float d, float w){ float e=max(d,0.0)/w; return exp(-e*e); }
float voro(vec2 p, out vec2 cell){ vec2 i=floor(p), f=fract(p); float d1=8.0, d2=8.0; cell=vec2(0.0);
  for(int y=-1;y<=1;y++) for(int x=-1;x<=1;x++){ vec2 g=vec2(float(x),float(y)); vec2 o=h22(i+g); float d=length(g+o-f);
    if(d<d1){ d2=d1; d1=d; cell=i+g; } else if(d<d2) d2=d; }
  return d2-d1; }
float sigBody(vec2 b, float r){ float y=b.y, L=uLength; float beyond=max(y-L,0.0); return length(vec2(b.x, beyond))/r*step(-1.0,y); }
// ---- SJK effect helpers (shared by the signatures; Creyon's round 7) ----
// Distance from p to a jagged lightning bolt from a to c: 10 kinks, displaced across the
// line by up to jag of its length, the kinks chosen by seed (strike anew by changing it).
float boltD(vec2 p, vec2 a, vec2 c, float seed, float jag){
  vec2 d = c - a; float L = max(length(d), 1e-3); vec2 n = vec2(-d.y, d.x)/L;
  float best = 1e3; vec2 prev = a;
  for (int i = 1; i <= 10; i++){
    float f = float(i)/10.0;
    float off = i == 10 ? 0.0 : (h11(seed*17.13 + float(i)*7.31) - 0.5)*jag*L*sin(f*3.1416);
    vec2 q = a + d*f + n*off;
    best = min(best, sdSeg(p, prev, q));
    prev = q;
  }
  return best;
}
// A bolt's light: a white-hot thread in a coloured glow, never thinner than a pixel.
vec3 boltLight(float dist, vec3 col, float px){
  return vec3(1.0)*halo(dist, max(px, 0.035))*1.6 + col*halo(dist, 0.22)*0.9 + col*halo(dist, 0.9)*0.18;
}
// A Storm-like loop: a bolt that leaves the blade at height y0, arcs out to reach units on
// side s and comes back in at y1, struck anew rate times a second.
vec3 arcLoop(vec2 b, float y0, float y1, float s, float reach, float seed, float rate, vec3 col, float px){
  float k = floor(uTime*rate + seed*13.0);
  vec2 a = vec2(0.0, y0), c = vec2(0.0, y1);
  vec2 m = vec2(s*reach*(0.75 + 0.25*h11(k + seed)), mix(y0, y1, 0.5) + (h11(k*1.7 + seed) - 0.5)*2.0);
  float d = min(boltD(b, a, m, k + seed, 0.45), boltD(b, m, c, k + seed + 0.5, 0.45));
  return boltLight(d, col, px);
}
// A helix strand wound round the blade: across (x) and how near the viewer it is (z, -1..1)
// at height y, turns per 10 units, phase turns, radius r.
vec2 helix(float y, float r, float turns, float phase){
  float a = 6.2832*(y*turns/10.0 + phase);
  return vec2(r*sin(a), cos(a));
}
// ---- 3D helpers (round 8): effects drawn along the true view ray ----
// In sig_vol(ro, rd, px) a signature gets the view ray in the blade's frame: ro the camera,
// rd its unit direction, y along the blade from the hilt, x across it to the camera's side,
// z toward the camera. Everything placed with these sits round the blade in depth and keeps
// its shape from every side, from above and in first person.
// Round 22 (Creyon: nothing may be seen through the blade when it is behind it): how hidden by
// the blade's body a point t along the ray is, 0 (seen) to 1 (hidden): the ray passes through the
// blade's body (its nearest approach to the axis within the blade) before reaching it.
// Round 23 (players' reports after the 2026.1021.1 release: hard-edged boxes and lines round
// the tip when a blade pointed at the camera): it fades in and out softly; it used to switch
// off at once when looking nearly along the blade.
// 31/10/2026 (Sol: the top-tier blades cost too much): what of it depends on the ray alone (where
// the ray passes the axis, how near, the tip's narrowing) is worked out once per pixel for the
// view ray (`labRayPrepare`, which saber.wgsl calls before sig_vol, sig_orbit and their solid
// parts) and read at every piece's point; a ray that never passes behind the body hides nothing,
// so the rest is skipped. A piece asking for another ray (a shifted one) works its own out. The
// same values as before. behindBlade only reads what was kept (writing it there kept the shader
// compiler from dropping calls whose result goes unused: World Shift drew at half speed).
vec3 bbRo = vec3(1e9);
vec3 bbRd = vec3(0.0);
float bbTa = 0.0;
float bbNear = 0.0;
float bbAlong = 0.0;
void labRayPrepare(vec3 ro, vec3 rd){
  bbRo = ro;
  bbRd = rd;
  bbTa = 0.0;
  bbNear = 0.0;
  float a = dot(rd.xz, rd.xz);
  bbAlong = smoothstep(0.01, 0.04, a);
  if (bbAlong <= 0.0) return;
  float ta = -dot(ro.xz, rd.xz)/a;
  float y = ro.y + rd.y*ta;
  vec2 off = ro.xz + rd.xz*ta;
  float d = length(off);
  float into = clamp((y - (uLength - 1.4))/1.4, 0.0, 1.0);
  float w = sqrt(max(1.0 - into*into, 0.0));
  float onBlade = smoothstep(-0.8, -0.2, y)*(1.0 - smoothstep(uLength - 0.15, uLength + 0.05, y));
  bbTa = ta;
  bbNear = (1.0 - smoothstep(0.35*w, 0.8*w + 0.02, d))*onBlade;
}
// The whole of it, from the ray alone: what labRayPrepare keeps, and what a piece asking for
// another ray (or a module without the cache) works out (written out twice rather than shared
// through out parameters, which made World Shift draw at a third of its speed on an RTX 5080).
float behindBladeFull(vec3 ro, vec3 rd, float t){
  float a = dot(rd.xz, rd.xz);
  float along = smoothstep(0.01, 0.04, a);
  if (along <= 0.0) return 0.0;
  float ta = -dot(ro.xz, rd.xz)/a;
  float y = ro.y + rd.y*ta;
  vec2 off = ro.xz + rd.xz*ta;
  float d = length(off);
  // what lies on the blade's own body (its axis, its glow) is never hidden by it
  vec3 p = ro + rd*t;
  float outside = smoothstep(0.5, 0.9, length(p.xz));
  // round 23: the body hiding narrows with the blade's rounded tip (a quarter circle over its
  // last 1.4 units) down to the point, so what passes behind the tip is hidden too (Sol's review:
  // ending the hiding before the tip let Exegol Rift's flare and Quasar's jet show through),
  // without the dark round end past the tip that a full-width hiding drew seen end on
  float into = clamp((y - (uLength - 1.4))/1.4, 0.0, 1.0);
  float w = sqrt(max(1.0 - into*into, 0.0));
  float onBlade = smoothstep(-0.8, -0.2, y)*(1.0 - smoothstep(uLength - 0.15, uLength + 0.05, y));
  return (1.0 - smoothstep(0.35*w, 0.8*w + 0.02, d))*onBlade*outside*smoothstep(ta + 0.15, ta + 0.7, t)*along;
}
// For a signature whose header says `@raycache off`, its module calls behindBladeFull in its place
// (blade_signatures.rs module_source): World Shift drew at a third of its speed reading what was
// kept, or even with the reading switched off by a constant, on an RTX 5080 (Vulkan).
float behindBlade(vec3 ro, vec3 rd, float t){
  if (ro.x == bbRo.x && ro.y == bbRo.y && ro.z == bbRo.z && rd.x == bbRd.x && rd.y == bbRd.y && rd.z == bbRd.z){
    if (bbNear <= 0.0 || bbAlong <= 0.0) return 0.0;
    vec3 p = ro + rd*t;
    float outside = smoothstep(0.5, 0.9, length(p.xz));
    // multiplied in this order: hideD magnifies the last digit a thousandfold where a piece is
    // nearly hidden
    return bbNear*outside*smoothstep(bbTa + 0.15, bbTa + 0.7, t)*bbAlong;
  }
  return behindBladeFull(ro, rd, t);
}
// A distance d as seen when h (behindBlade) of it is hidden by the blade: unchanged at 0, out of
// reach at 1, and in between the piece narrows and dims, so whatever passes behind the blade fades
// out over the blade's soft edge. Round 23: adding 1e3*h hid a piece wherever h was more than a
// thousandth, a hard cut along the blade's body that showed as a box round the tip when the blade
// pointed at the camera.
float hideD(float d, float h){ return d/max(1.0 - h, 1e-3) + h; }
// The distance from the ray to point p, as seen (a point the blade hides is far off); t
// returns how far along the ray that is.
float rayPtRaw(vec3 ro, vec3 rd, vec3 p, out float t){
  t = max(dot(p - ro, rd), 0.0);
  return length(ro + rd*t - p);
}
float rayPt(vec3 ro, vec3 rd, vec3 p, out float t){
  float d = rayPtRaw(ro, rd, p, t);
  return hideD(d, behindBlade(ro, rd, t));
}
// The distance from the ray to the segment a-b; s returns where on it (0 at a, 1 at b); the
// raw one ignores the blade's body (for bounds and the blade's own axis).
float raySegRaw(vec3 ro, vec3 rd, vec3 a, vec3 b, out float s){
  vec3 ba = b - a, w = ro - a;
  float bd = dot(rd, ba), bb = dot(ba, ba), dw = dot(rd, w), bw = dot(ba, w);
  float den = max(bb - bd*bd, 1e-6);
  s = clamp((bw - bd*dw)/den, 0.0, 1.0);
  vec3 q = a + ba*s;
  float t = max(dot(q - ro, rd), 0.0);
  vec3 pr = ro + rd*t;
  s = clamp(dot(pr - a, ba)/max(bb, 1e-6), 0.0, 1.0);
  return length(pr - (a + ba*s));
}
float raySeg(vec3 ro, vec3 rd, vec3 a, vec3 b, out float s){
  float d = raySegRaw(ro, rd, a, b, s);
  vec3 q = a + (b - a)*s;
  return hideD(d, behindBlade(ro, rd, max(dot(q - ro, rd), 0.0)));
}
// Where the ray crosses the plane across the blade at height h: xz there, and t (negative:
// never, or behind the camera).
vec3 rayAt(vec3 ro, vec3 rd, float h){
  float t = abs(rd.y) > 1e-5 ? (h - ro.y)/rd.y : -1.0;
  vec3 p = ro + rd*max(t, 0.0);
  return vec3(p.x, p.z, t);
}
// Where the ray is within R of the blade's axis and between heights y0 and y1: the span of
// t (empty when x >= y).
vec2 raySpan(vec3 ro, vec3 rd, float R, float y0, float y1){
  float a = dot(rd.xz, rd.xz), b = dot(ro.xz, rd.xz), c = dot(ro.xz, ro.xz) - R*R;
  vec2 s = vec2(0.0, 1e5);
  if (a < 1e-6){
    if (c > 0.0) return vec2(1.0, 0.0);
  } else {
    float h = b*b - a*c;
    if (h < 0.0) return vec2(1.0, 0.0);
    h = sqrt(h);
    s = vec2((-b - h)/a, (-b + h)/a);
  }
  if (abs(rd.y) > 1e-6){
    float ta = (y0 - ro.y)/rd.y, tb = (y1 - ro.y)/rd.y;
    s = vec2(max(s.x, min(ta, tb)), min(s.y, max(ta, tb)));
  } else if (ro.y < y0 || ro.y > y1){
    return vec2(1.0, 0.0);
  }
  s.x = max(s.x, 0.0);
  return s;
}
// round 19: `p` turned a (turns) round the blade's axis, for a piece laid out in the blade's
// picture (x across, y up) to stand at its own place round the blade in three dimensions.
vec3 turnY(vec3 p, float a){ float c = cos(6.2832*a), s = sin(6.2832*a); return vec3(p.x*c + p.z*s, p.y, -p.x*s + p.z*c); }
// A point round the blade: radius r, angle a (turns), height y.
vec3 around(float r, float a, float y){ return vec3(r*sin(6.2832*a), y, r*cos(6.2832*a)); }
// The blade itself in three dimensions: a white-hot core and a coloured glow round the
// axis from just inside the hilt to the tip. It fills the hilt's mouth as a stock blade
// does, seen from any side, from above and in first person. w is the core's half width.
vec3 blade3(vec3 ro, vec3 rd, float px, vec3 col, float w, float top){
  float s;
  float d = raySegRaw(ro, rd, vec3(0.0, -0.6, 0.0), vec3(0.0, top, 0.0), s);
  return vec3(1.0)*exp(-pow(d/max(w, px), 2.0))*1.4 + col*halo(d, w*2.2)*0.75 + col*halo(d, w*6.0)*0.18;
}
// A lightning bolt in three dimensions from a to c: 10 kinks, each pushed off the line in
// both directions across it by up to jag of its length.
float bolt3(vec3 ro, vec3 rd, vec3 a, vec3 c, float seed, float jag, float px){
  vec3 d = c - a; float L = max(length(d), 1e-3);
  // 31/10/2026 (Sol: the top-tier blades cost too much): every point of the bolt lies within
  // half its length and 0.71 of its zigzag (jag*L) of its middle, so a ray passing further off
  // than that plus six units and three pixels is further than that from all of it, where the
  // widest glow a caller gives a bolt (1.5) or a pixel-wide one is under a ten-thousandth: such a
  // ray skips the ten segments.
  float tm;
  if (rayPtRaw(ro, rd, (a + c)*0.5, tm) - L*(0.5 + 0.71*jag) > max(6.0, 3.0*px)) return 1e3;
  vec3 n1 = normalize(cross(d, abs(d.y) < 0.9*L ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
  vec3 n2 = cross(d/L, n1);
  float best = 1e3; vec3 prev = a; float s;
  for (int i = 1; i <= 10; i++){
    float f = float(i)/10.0;
    float k = i == 10 ? 0.0 : sin(f*3.1416)*jag*L;
    vec3 q = a + d*f + (n1*(h11(seed*17.13 + float(i)*7.31) - 0.5) + n2*(h11(seed*5.71 + float(i)*3.17) - 0.5))*k;
    best = min(best, raySeg(ro, rd, prev, q, s));
    prev = q;
  }
  return best;
}
// A soft cloud of light: march the ray through a smooth density inside radius R between
// heights y0 and y1 in n steps (the caller sums its own field; this gives the steps).
// Returns the span and the step length in z.
vec3 marchSteps(vec3 ro, vec3 rd, float R, float y0, float y1, float n){
  vec2 s = raySpan(ro, rd, R, y0, y1);
  return vec3(s, max(s.y - s.x, 0.0)/n);
}
// ---- round 9: the big effects (Creyon: "dramatic and beautiful, like beams, strays, rays") ----
// Every one is placed in the blade's own frame (fixed to the hilt), sized in units, and fades
// in and out smoothly; the loops have constant bounds and stop at `count`.
// How near the ray passes the blade's axis between heights y0 and y1 (for early outs).
float fxNear(vec3 ro, vec3 rd, float y0, float y1){
  float s;
  return raySegRaw(ro, rd, vec3(0.0, y0, 0.0), vec3(0.0, y1, 0.0), s);
}
// The ray's distance to a ring of radius R round the blade at height y: the nearer of where
// it crosses the ring's plane and the ring's cylinder (smooth from any side).
float ringD(vec3 ro, vec3 rd, float y, float R){
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
// A star flare at p facing the camera: a white-hot heart of radius size and n thin rays of
// length len, turned by spin (radians).
vec3 fxFlare(vec3 ro, vec3 rd, vec3 p, float px, float size, float len, float n, float spin, vec3 col){
  float t;
  float d = rayPt(ro, rd, p, t);
  // round 23: skipped only past its widest halo (size*8), which fades to nothing before the cut
  // (at size*4 it was still a tenth as bright and ended at a hard ring round the flare; Sol's
  // review: size*18 cost too much, size*12 with the taper below is as clean)
  if (d > len + size*12.0) return vec3(0.0);
  vec3 q = ro + rd*t - p;
  vec3 u = normalize(cross(rd, abs(rd.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
  vec3 v = cross(rd, u);
  vec2 e = vec2(dot(q, u), dot(q, v));
  float r = length(e);
  float k = 6.2832/max(n, 1.0);
  float aa = mod(atan(e.y, e.x) + spin + k*0.5, k) - k*0.5;
  float across = abs(r*sin(aa)), along = r*cos(aa);
  float w = max(px, 0.03 + 0.012*along);
  float ray = exp(-pow(across/w, 2.0))*exp(-max(along, 0.0)/max(len, 0.01)*2.6)*step(0.0, along);
  float wide = halo(d, size*8.0)*(1.0 - smoothstep(len + size*7.0, len + size*12.0, d));
  return (vec3(1.0)*(halo(d, max(size, px))*1.8 + ray*0.9) + col*(halo(d, size*3.0)*0.7 + ray*0.8 + wide*0.15))*roomAt(p);
}
// Beams of light shooting out of the blade between heights y0 and y1: count of them (up to
// 24), each up to len long, flaring out and fading rate times a second, seed chooses them.
vec3 fxRays(vec3 ro, vec3 rd, float px, float y0, float y1, float count, float len, float rate, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  // round 23: past the beams' widest glow (nine of their widths), which ended at a hard edge
  if (fxNear(ro, rd, y0, y1) > len*1.6 + 5.0) return acc;
  float s;
  for (int i = 0; i < 24; i++){
    float fi = float(i);
    if (fi >= count) break;
    float cyc = uTime*rate*(0.8 + 0.4*h11(fi*3.3 + seed)) + h11(fi*7.13 + seed);
    float k = floor(cyc), q = fract(cyc);
    float y = mix(y0, y1, h11(k*3.1 + fi*1.7 + seed));
    float a = h11(k*5.7 + fi*2.3 + seed);
    float tilt = (h11(k*2.9 + fi + seed) - 0.5)*0.9;
    vec3 dir = normalize(vec3(sin(6.2832*a), tilt, cos(6.2832*a)));
    float lr = min(len*(0.55 + 0.6*h11(k*1.3 + fi + seed)), 0.85*uReach)*smoothstep(0.0, 0.3, q);
    vec3 a0 = vec3(0.0, y, 0.0) + dir*0.35;
    float d = raySeg(ro, rd, a0, a0 + dir*max(lr, 0.01), s);
    float env = sin(3.1416*q);
    float w = mix(0.3, 0.07, s);
    acc += (vec3(1.0)*halo(d, max(w*0.45, px))*1.4 + col*halo(d, w*3.0)*0.95 + col*halo(d, w*9.0)*0.22)*(1.0 - 0.75*s)*env;
  }
  return acc;
}
// A bolt's light (boltLight) faded to nothing 1.7 from it, so fxArcs can skip a bolt past that
// without a seam: its widest glow is a fortieth as bright there.
vec3 arcGlow(float dist, vec3 col, float px){
  return boltLight(dist, col, px)*(1.0 - smoothstep(1.1, 1.7, dist));
}
// Lightning in three dimensions: count (up to 10) bolts that leave the blade between y0 and
// y1, arc out up to reach units on every side and come back in, with a fork, struck anew
// rate times a second (Sol's Storm, all round the blade).
vec3 fxArcs(vec3 ro, vec3 rd, float px, float y0, float y1, float count, float reach, float rate, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  // round 23: past the forks' reach (half again the bolt's, which the blade's room caps at 0.6 of
  // it) and their glow, which ended at a hard edge
  if (fxNear(ro, rd, y0, y1) > min(reach, 0.6*uReach)*1.6 + 3.0) return acc;
  float t;
  for (int i = 0; i < 10; i++){
    float fi = float(i);
    if (fi >= count) break;
    float cyc = uTime*rate*(0.7 + 0.6*h11(fi*1.9 + seed)) + h11(fi*3.3 + seed)*5.0;
    float k = floor(cyc), q = fract(cyc);
    float ya = mix(y0, y1, h11(k*1.9 + fi*2.7 + seed));
    float yb = min(ya + 3.0 + 6.0*h11(k*2.3 + fi + seed), y1 + 2.0);
    float a = h11(k*4.1 + fi*2.0 + seed);
    float R = min(reach, 0.6*uReach)*(0.45 + 0.55*h11(k*7.7 + fi + seed));
    vec3 mid = around(R, a, mix(ya, yb, 0.5) + (h11(k*9.1 + fi) - 0.5)*2.0);
    // round 23 (Sol's review: timed on Creyon's RTX 5070, the arc blades cost a fifth to a third
    // more once their glow faded out instead of being cut): each of the arc's three bolts (its two
    // halves and the fork) is drawn only near its own middle. A bolt's points lie within half its
    // length of its middle (its zigzag reaches less aside than that), and its glow is made to end
    // 1.7 past them (arcGlow), so a pixel further off would draw nothing from it.
    if (rayPtRaw(ro, rd, mid, t) > R*1.25 + (yb - ya)*0.65 + 2.8) continue;
    float env = (1.0 - q)*(1.0 - q)*(0.6 + 0.4*step(0.5, fract(q*9.0 + fi)));
    vec3 ea = around(0.25, a, ya), eb = around(0.25, a, yb);
    vec3 fork = mid + around(R*0.55, a + 0.12*(h11(k + fi) - 0.5), (h11(k*3.3 + fi) - 0.5)*3.0);
    float d = 1e3, d2 = 1e3;
    if (rayPtRaw(ro, rd, (ea + mid)*0.5, t) < 0.55*length(mid - ea) + 1.8 + px)
      d = bolt3(ro, rd, ea, mid, k + fi*1.37 + seed, 0.5, px);
    if (rayPtRaw(ro, rd, (mid + eb)*0.5, t) < 0.55*length(eb - mid) + 1.8 + px)
      d = min(d, bolt3(ro, rd, mid, eb, k + fi*1.37 + seed + 0.5, 0.5, px));
    if (rayPtRaw(ro, rd, (mid + fork)*0.5, t) < 0.55*length(fork - mid) + 1.8 + px)
      d2 = bolt3(ro, rd, mid, fork, k*2.1 + fi + seed, 0.6, px);
    acc += (arcGlow(d, col, px) + arcGlow(d2, col, px)*0.55)*env;
  }
  return acc;
}
// Sparks spraying off the blade between y0 and y1: count (up to 32) streaks flying out up
// to len units, white-hot then cooling to col, rate cycles a second.
vec3 fxSparks(vec3 ro, vec3 rd, float px, float y0, float y1, float count, float len, float rate, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  if (fxNear(ro, rd, y0, y1) > len + 1.5) return acc;
  float s;
  for (int i = 0; i < 32; i++){
    float fi = float(i);
    if (fi >= count) break;
    float cyc = uTime*rate*(0.7 + 0.6*h11(fi*2.1 + seed)) + h11(fi*5.9 + seed);
    float k = floor(cyc), q = fract(cyc);
    float y = mix(y0, y1, h11(k*2.7 + fi*1.3 + seed));
    float a = h11(k*6.3 + fi*0.7 + seed);
    vec3 dir = normalize(vec3(sin(6.2832*a), 0.2 + 0.9*(h11(k*1.1 + fi + seed) - 0.3), cos(6.2832*a)));
    float v = min(len, 0.85*uReach)*(0.5 + 0.5*h11(k*4.4 + fi + seed));
    vec3 o = around(0.3, a, y);
    float e1 = 1.0 - exp(-q*3.0), e0 = 1.0 - exp(-max(q - 0.07, 0.0)*3.0);
    float d = raySeg(ro, rd, o + dir*v*e0, o + dir*v*e1, s);
    float life = 1.0 - smoothstep(0.55, 1.0, q);
    acc += mix(vec3(1.0, 0.97, 0.9), col, smoothstep(0.0, 0.6, q))*(halo(d, max(0.05, px))*2.0 + halo(d, 0.28)*0.3)*life;
  }
  return acc;
}
// Coils of light wound round the blade from y0 to y1 at radius R, turns per 10 units,
// travelling up at speed; knots of light run along them. count (up to 4) strands.
vec3 fxCoils(vec3 ro, vec3 rd, float px, float y0, float y1, float R, float turns, float speed, float count, vec3 col){
  vec3 acc = vec3(0.0);
  // round 23: past the coils' widest glow (it ended at a hard edge, R + 1.5)
  if (fxNear(ro, rd, y0, y1) > R + 2.5) return acc;
  float s;
  for (int j = 0; j < 4; j++){
    float fj = float(j);
    if (fj >= count) break;
    float best = 1e3, bf = 0.0;
    vec3 prev = vec3(0.0);
    for (int i = 0; i <= 24; i++){
      float f = float(i)/24.0;
      float y = mix(y0, y1, f);
      vec3 p = around(R*(0.85 + 0.15*sin(f*12.0 + uTime*2.0 + fj)), y*turns/10.0 - uTime*speed + fj/max(count, 1.0), y);
      if (i > 0){
        float d = raySeg(ro, rd, prev, p, s);
        if (d < best){ best = d; bf = (float(i) - 1.0 + s)/24.0; }
      }
      prev = p;
    }
    float knot = pow(0.5 + 0.5*sin(bf*40.0 - uTime*9.0 + fj*2.0), 6.0);
    float ends = smoothstep(0.0, 0.08, bf)*(1.0 - smoothstep(0.9, 1.0, bf));
    acc += (vec3(1.0)*halo(best, max(0.05, px))*(0.5 + 1.5*knot) + col*halo(best, 0.3)*(0.5 + knot) + col*halo(best, 1.0)*0.12)*ends;
  }
  return acc;
}
// Motes (dust, embers, sparks of light) drifting up round the blade between radii r0 and r1:
// count (up to 40), each a soft point that twinkles.
vec3 fxMotes(vec3 ro, vec3 rd, float px, float y0, float y1, float r0, float r1, float count, float speed, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  if (fxNear(ro, rd, y0, y1) > r1 + 1.0) return acc;
  float t;
  for (int i = 0; i < 40; i++){
    float fi = float(i);
    if (fi >= count) break;
    float f = fract(uTime*speed*(0.6 + 0.8*h11(fi*2.9 + seed)) + h11(fi*5.1 + seed));
    float y = mix(y0 - 1.0, y1 + 2.0, f);
    vec3 p = around(mix(r0, r1, h11(fi*3.7 + seed)), h11(fi*1.9 + seed) + uTime*0.04 + 0.08*sin(uTime*0.7 + fi), y);
    float d = rayPt(ro, rd, p, t);
    float tw = 0.55 + 0.45*sin(uTime*5.0 + fi*2.3);
    acc += col*(halo(d, max(0.07, px))*1.8 + halo(d, 0.35)*0.2)*tw*smoothstep(0.0, 0.1, f)*(1.0 - smoothstep(0.8, 1.0, f))*roomAt(p);
  }
  return acc;
}
// A shock ring racing out from the blade at height y: q its age (0 to 1), out to R.
vec3 fxShock(vec3 ro, vec3 rd, float px, float y, float q, float R, vec3 col){
  if (q <= 0.0 || q >= 1.0) return vec3(0.0);
  float Rr = 0.5 + R*(1.0 - pow(1.0 - q, 2.0));
  float d = ringD(ro, rd, y, Rr);
  float env = (1.0 - q)*smoothstep(0.0, 0.05, q)*roomFade(Rr + 0.8);
  return (vec3(1.0)*halo(d, max(0.07, px))*1.4 + col*halo(d, 0.45)*0.7 + col*halo(d, 1.4)*0.15)*env;
}
// A wide soft aura round the blade from y0 to y1: its light reaching w units out.
vec3 fxAura(vec3 ro, vec3 rd, float y0, float y1, float w, vec3 col){
  float d = fxNear(ro, rd, y0, y1);
  return col*(halo(d, w)*0.35 + halo(d, w*0.4)*0.25);
}
// ---- round 11: small helpers for the lower tiers ----
// The glow's level and the glow at that level in colour c (a tint keeping Sol's shape).
float lvl(vec3 g){ return max(g.r, max(g.g, g.b)); }
vec3 recol(vec3 g, vec3 c){ return lvl(g)*c; }
// Bands of light travelling along the blade: k bands a unit, moving at v units a second.
float bands(float y, float k, float v){ return 0.5 + 0.5*sin(y*k*6.2832 - uTime*v*k*6.2832); }
// Falling streaks (rain, snow, ash) in the blade's picture: count of them (up to 32) between
// heights y0 and y1, reaching r across, falling at v, slanted by lean.
vec3 fxFall(vec3 ro, vec3 rd, float px, float y0, float y1, float r, float count, float v, float len, float lean, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  // round 23: the streaks start up to 2 units past y0 and y1 and reach len further (leaning
  // lean*len aside): skipped only outside all that (Lightspeed Ram's rising streaks were cut
  // above the tip)
  if (fxNear(ro, rd, y0 - 2.0 + min(len, 0.0), y1 + 2.0 + max(len, 0.0)) > r + abs(lean*len) + 1.0) return acc;
  float s;
  for (int i = 0; i < 32; i++){
    float fi = float(i);
    if (fi >= count) break;
    float f = fract(uTime*v/(y1 - y0 + 4.0)*(0.7 + 0.6*h11(fi*2.3 + seed)) + h11(fi*5.9 + seed));
    float y = mix(y1 + 2.0, y0 - 2.0, f);
    vec3 p = around(0.8 + (min(r, 0.9*uReach) - 0.8)*h11(fi*3.1 + seed), h11(fi*7.7 + seed), y);
    vec3 q = p + vec3(lean, 1.0, 0.0)*len;
    float d = raySeg(ro, rd, p, q, s);
    acc += col*(halo(d, max(0.04, px))*1.2 + halo(d, 0.2)*0.2)*(1.0 - 0.6*s);
  }
  return acc;
}
// ---- round 11: every effect can be switched off ----
// 1 while the signature's own part k (its `// @part` line k, from 0) is on, 0 when its wearer
// switched it off in the Collection.
float part(float k){ return 1.0 - mod(floor(uParts/exp2(k) + 0.001), 2.0); }
// ---- round 19: nothing is ever cut at the quad's edge ----
// How much of what lies r units from the blade's axis shows: all of it well inside the reach
// the blade's quad holds without fading (`uReach`), fading smoothly to nothing before it
// (Creyon: Reactor's shock ring was cut off where it left that room).
float roomFade(float r){ return 1.0 - smoothstep(0.72*uReach, 0.97*uReach, r); }
float roomAt(vec3 p){ return roomFade(length(p.xz)); }
// ---- round 23: sharp patterns without stair steps (players' reports after 2026.1021.1) ----
// A step from 0 to 1 at e, smoothed over w (a pixel, in x's units): a pattern's edge stays sharp
// but no longer stair-steps or flickers as the blade moves.
float aaStep(float e, float x, float w){ return smoothstep(e - 0.5*w, e + 0.5*w, x); }
// How far a pixel is into the next cell of u (cells one long), over the cell's last w (a pixel in
// u's units, and at least a twelfth of the cell, as px is the pixel across the blade and a pixel
// along it is longer where the blade is seen end on): blending a cell's value into the next one's
// by it keeps blocks and stripes sharp.
float cellMix(float u, float w){ return smoothstep(1.0 - clamp(w, 0.08, 0.5), 1.0, fract(u)); }
// A square wave of u, 0 for the first half of each one-long cell and 1 for the second, its two
// edges smoothed over w (a pixel in u's units, at least a twelfth of the cell, as cellMix).
float aaSquare(float u, float w){ float f = fract(u); w = clamp(w, 0.08, 0.25); return aaStep(0.5, f, w)*(1.0 - smoothstep(1.0 - w, 1.0, f)); }
// ---- round 12: richer pieces for the lower tiers (each blade sets its own pattern) ----
// Soft curling smoke round the blade between heights y0 and y1, w units to each side at its
// thickest, rising at `rise` units a second (negative sinks): warped smooth noise with no hard
// edge (Creyon's round 12: "the smoke looks so squary"). Round 19 (Creyon: seen from above, the
// smoke was a flat sheet following the camera): a cloud in three dimensions, the ray marched
// through it in six steps, as bright side on as the old sheet.
float fxSmokeAt(vec3 p, float w, float y0, float y1, float rise, float seed){
  float ax = length(p.xz);
  vec2 q = vec2((p.x + 0.62*p.z)*0.32, p.y*0.12 - uTime*rise*0.12) + vec2(seed*7.3, seed*3.1);
  q += vec2(0.0, p.z*0.09 - p.x*0.05);
  vec2 warp = vec2(vn(rot(0.6)*q*0.9 + vec2(0.0, uTime*0.07)), vn(rot(2.1)*q*0.9 + vec2(5.2, -uTime*0.06)));
  vec2 r = q + 1.3*warp;
  float n = 0.6*vn(rot(0.35)*r) + 0.4*vn(rot(1.7)*r*2.1 + vec2(3.3, 1.1) + vec2(p.z*0.2, -p.x*0.2));
  float shape = exp(-pow(ax/w, 2.0))*smoothstep(y0 - 3.0, y0 + 2.0, p.y)*(1.0 - smoothstep(y1 - 7.0, y1 - 1.0, p.y));
  return smoothstep(0.35, 0.8, n)*shape;
}
// Round 23 (players' reports after 2026.1021.1: a hard-edged dome in the smoke seen along the
// blade): a step at most about a third of the cloud's width, up to 12 of them (16 cost the smoke
// blades up to a sixth more seen along them, Sol's review), so a long path through it (looking
// along the blade) is not sampled six units apart; seen from the side the path is short and it
// still takes six.
vec3 fxSmoke(vec3 ro, vec3 rd, float y0, float y1, float w, float rise, float seed, vec3 col){
  vec2 span = raySpan(ro, rd, w*2.4, y0 - 3.0, y1);
  if (span.x >= span.y) return vec3(0.0);
  float n = clamp(ceil((span.y - span.x)/(0.8*w)), 6.0, 12.0);
  float dt = (span.y - span.x)/n;
  float acc = 0.0;
  for (int i = 0; i < 12; i++){
    if (float(i) >= n) break;
    float ts = span.x + (float(i) + 0.5)*dt;
    acc += fxSmokeAt(ro + rd*ts, w, y0, y1, rise, seed)*(1.0 - behindBlade(ro, rd, ts));
  }
  return col*acc*dt/(1.77*w)*0.9;
}
// Ribbons of energy flowing along the blade: count (up to 4) wavy strands alternating sides,
// R units out at most, `wave` their waviness, pulses of light running up them at `speed`.
vec3 fxStreams(vec3 ro, vec3 rd, float px, float y0, float y1, float count, float R, float wave, float speed, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  if (fxNear(ro, rd, y0, y1) > R + 1.5) return acc;
  float s;
  for (int j = 0; j < 4; j++){
    float fj = float(j);
    if (fj >= count) break;
    float best = 1e3, bf = 0.0;
    vec3 prev = vec3(0.0);
    // round 19: each strand on its own side all round the blade, in three dimensions (it lay
    // in one flat sheet)
    float side = fj/max(count, 1.0) + 0.13*seed;
    for (int i = 0; i <= 14; i++){
      float f = float(i)/14.0;
      float y = mix(y0, y1, f);
      float x = 0.45 + (min(R, 0.9*uReach) - 0.45)*(0.5 + 0.5*sin(y*wave + fj*1.7 + seed + uTime*(0.5 + 0.2*fj)));
      vec3 p = around(x, side + 0.05*sin(y*0.15 + uTime*0.3 + fj), y);
      if (i > 0){
        float d = raySeg(ro, rd, prev, p, s);
        if (d < best){ best = d; bf = (float(i) - 1.0 + s)/14.0; }
      }
      prev = p;
    }
    float pulse = pow(0.5 + 0.5*sin(bf*18.0 - uTime*speed + fj*2.1 + seed), 4.0);
    float ends = smoothstep(0.0, 0.1, bf)*(1.0 - smoothstep(0.85, 1.0, bf));
    acc += (vec3(1.0)*halo(best, max(0.035, px))*(0.25 + 1.1*pulse) + col*halo(best, 0.22)*(0.35 + 0.8*pulse) + col*halo(best, 0.9)*0.07)*ends;
  }
  return acc;
}
// Flakes of light (snow, ash, petals, leaves of light) drifting down round the blade: count
// (up to 24) soft flat ovals `size` long and `aspect` as wide, falling at v, turning at spin,
// swaying `drift` units, spread up to `spread` units to each side.
vec3 fxFlakes(vec3 ro, vec3 rd, float px, float y0, float y1, float spread, float count, float v, float size, float aspect, float spin, float drift, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  // round 23: skipped only past where a flake's soft halo has faded (a flake sways `drift` past
  // `spread`, and its halo reaches about six sizes): cut there, it drew a hard dome round the
  // blade seen along it (Yavin Vines, players' reports after 2026.1021.1)
  if (fxNear(ro, rd, y0 - 3.0, y1) > spread + abs(drift) + size*6.0 + 1.0) return acc;
  float ta;
  for (int i = 0; i < 24; i++){
    float fi = float(i);
    if (fi >= count) break;
    float f = fract(uTime*v/(y1 - y0 + 4.0)*(0.7 + 0.6*h11(fi*2.7 + seed)) + h11(fi*6.1 + seed));
    float y = mix(y1, y0 - 3.0, f);
    // round 19: all round the blade in three dimensions (they drifted in one flat sheet),
    // each flake a small oval facing the camera
    float rr = min(0.6 + (spread - 0.6)*h11(fi*3.9 + seed) + drift*sin(uTime*0.9 + fi*2.0), 0.95*uReach);
    vec3 p = around(max(rr, 0.4), h11(fi*4.3 + seed) + 0.03*sin(uTime*0.5 + fi), y);
    float d = rayPt(ro, rd, p, ta);
    if (d > size*6.0 + px) continue;
    vec3 q = ro + rd*ta - p;
    vec3 fu = normalize(cross(rd, abs(rd.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
    vec2 e = rot(uTime*spin + fi*1.7)*vec2(dot(q, fu), dot(q, cross(rd, fu)));
    float pd = length(e/vec2(size, size*aspect));
    acc += col*(halo(pd - 1.0, 0.3)*0.8 + halo(d, size*2.5)*0.1)*smoothstep(0.0, 0.1, f)*(1.0 - smoothstep(0.85, 1.0, f));
  }
  return acc;
}
// Waves of light travelling up (or down, v < 0) the blade as rings round it, `count` at a
// time (up to 6), R across, each fading as it goes.
vec3 fxRingWaves(vec3 ro, vec3 rd, float px, float y0, float y1, float count, float R, float v, float seed, vec3 col){
  vec3 acc = vec3(0.0);
  if (fxNear(ro, rd, y0, y1) > R + 1.5) return acc;
  for (int i = 0; i < 6; i++){
    float fi = float(i);
    if (fi >= count) break;
    float q = fract(uTime*v/(y1 - y0) + fi/max(count, 1.0) + seed);
    if (v < 0.0) q = 1.0 - q;
    float y = mix(y0, y1, q);
    float d = ringD(ro, rd, y, min(R, 0.9*uReach)*(0.7 + 0.3*sin(3.1416*q)));
    float env = sin(3.1416*q);
    acc += (vec3(1.0)*halo(d, max(0.04, px))*0.7 + col*halo(d, 0.3)*0.6 + col*halo(d, 1.0)*0.08)*env;
  }
  return acc;
}
// ---- round 17: what circles the blade goes behind it ----
// How much of what is at `at` the blade's body leaves seen along this ray (in the hilt's frame):
// all of it in front of the blade, none of it behind (Creyon: Balance's orb of shadow showed
// through the blade).
float bladeSeen(vec3 ro, vec3 rd, vec3 at){
  float L = uLength, sAx, tAx, tAt;
  raySegRaw(ro, rd, vec3(0.0), vec3(0.0, L, 0.0), sAx);
  float dAx = rayPtRaw(ro, rd, vec3(0.0, sAx*L, 0.0), tAx);
  rayPt(ro, rd, at, tAt);
  // round 23: fading in behind the axis over a little depth, not at a hard step
  return 1.0 - (1.0 - smoothstep(0.35, 0.9, dAx))*smoothstep(tAx - 0.4, tAx + 0.4, tAt);
}
