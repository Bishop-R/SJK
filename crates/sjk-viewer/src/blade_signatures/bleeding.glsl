// Bleeding Kyber: a kyber crystal fighting the dark side's corruption, as a blade (Creyon's first
// Blade Lab design, rebuilt in round 9 as Sol builds his). The blade is Sol's glow and core from
// the file, pure white, and a red corruption bleeds up it from the hilt, cracking through the
// light in jagged veins, then is pushed back down by the crystal before it rises again. Round
// it in 3D the fight goes on: forked Sith lightning lashes out from the bleeding part, striking
// with a flash, and small arcs crawl over it, drops of red light swell on it and fall in long
// streaks, hot embers crack off its front in curving arcs, and where the white holds, beams
// of pure light flash out and push back. A swing helps the light push; a clash lets the
// corruption surge up with a red shock ring.
// @part Corruption veins
// @part Red drops
// @part Pure light
// @room 10
// @palette {"corona.rim_cool":[0.6,0.7,0.95],"corona.rim_hot":[0.95,0.97,1.0],"corona.inner":[1.0,1.0,1.0],"core.white":[1.0,1.0,1.0],"core.fringe_cool":[0.85,0.9,1.0],"core.fringe_hot":[1.0,0.95,0.95],"light.color":[1.0,0.85,0.88],"trail":[1.0,0.75,0.8]}

const vec3 BK_RED = vec3(1.0, 0.04, 0.06);
const vec3 BK_WHITE = vec3(0.78, 0.88, 1.0);
// how high the corruption has climbed (units): rising and pushed back on a slow tide
float bk_level(){
  float tide = 0.22 + 0.48*(0.5 + 0.5*sin(uTime*0.4));
  return (tide + 0.35*clamp(uClash, 0.0, 1.0) - 0.15*clamp(uSwing, 0.0, 1.0))*uLength;
}
// how corrupted the blade is at height y (with its jagged front)
float bk_mask(float y){
  float front = bk_level() + 1.5*(vn(vec2(y*0.5, uTime*0.7)) - 0.5);
  return 1.0 - smoothstep(front - 1.5, front + 0.3, y);
}

// round 14 (Creyon: "the lightning looks really poor"): a forked bolt of Sith lightning from a
// to c, 14 kinks of large jag with a fine crackle on top, and two forks breaking off it; the
// distance from the ray to the nearest of them
float bk_bolt(vec3 ro, vec3 rd, vec3 a, vec3 c, float seed, float jag){
  vec3 d = c - a; float L = max(length(d), 1e-3);
  vec3 n1 = normalize(cross(d, abs(d.y) < 0.9*L ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
  vec3 n2 = cross(d/L, n1);
  float best = 1e3, s;
  vec3 prev = a, f1 = a, f2 = a;
  for (int i = 1; i <= 14; i++){
    float f = float(i)/14.0;
    float big = i == 14 ? 0.0 : sin(f*3.1416)*jag*L;
    float fine = i == 14 ? 0.0 : 0.05*L;
    vec3 q = a + d*f
      + (n1*(h11(seed*17.13 + float(i)*7.31) - 0.5) + n2*(h11(seed*5.71 + float(i)*3.17) - 0.5))*big
      + (n1*(h11(seed*3.9 + float(i)*13.7) - 0.5) + n2*(h11(seed*8.3 + float(i)*11.1) - 0.5))*fine;
    best = min(best, raySeg(ro, rd, prev, q, s));
    if (i == 5) f1 = q;
    if (i == 9) f2 = q;
    prev = q;
  }
  // the forks: shorter bolts breaking off sideways from two kinks
  vec3 o1 = (n1*(h11(seed*2.3) - 0.5) + n2*(h11(seed*4.7) - 0.5))*2.0;
  vec3 o2 = (n1*(h11(seed*6.1) - 0.5) + n2*(h11(seed*9.9) - 0.5))*2.0;
  vec3 e1 = f1 + d*0.28 + o1*L*0.35;
  vec3 e2 = f2 + d*0.22 + o2*L*0.3;
  vec3 p1 = f1, p2 = f2;
  for (int i = 1; i <= 5; i++){
    float f = float(i)/5.0;
    float k = i == 5 ? 0.0 : sin(f*3.1416)*jag*0.35*L;
    vec3 q1 = mix(f1, e1, f) + (n1*(h11(seed*31.7 + float(i)*5.3) - 0.5) + n2*(h11(seed*23.1 + float(i)*2.9) - 0.5))*k;
    vec3 q2 = mix(f2, e2, f) + (n1*(h11(seed*41.3 + float(i)*4.1) - 0.5) + n2*(h11(seed*37.7 + float(i)*6.7) - 0.5))*k;
    best = min(best, min(raySeg(ro, rd, p1, q1, s), raySeg(ro, rd, p2, q2, s)*1.25));
    p1 = q1; p2 = q2;
  }
  return best;
}

vec3 sig_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float lv = bk_level();
  float y0 = L*0.18, y1 = L*0.97;
  vec3 col = vec3(0.0), red = vec3(0.0);
  if (fxNear(ro, rd, 0.0, L) > 10.0) return col;
  float ta;
  // ---- Sith lightning (round 16, Creyon: it looked like drawings) ----
  // the corruption's lightning crawls over the bleeding part, hugging the blade: arcs leap from
  // one place on it to another a few units up, bowing out a little, flickering fast and glowing
  // hot, a crackle of many at once; now and then one lashes out further, forking, with a flash
  float ylo = max(y0, 1.0), yhi = max(lv, ylo + 3.0);
  for (int i = 0; i < 7; i++){
    float fi = float(i);
    float cyc = t*(2.2 + 1.3*h11(fi*2.7)) + h11(fi*3.1)*5.0;
    float k = floor(cyc), q = fract(cyc);
    float ya = mix(ylo, yhi - 1.0, h11(k*2.3 + fi));
    float yb = min(ya + 1.5 + 3.0*h11(k*4.1 + fi), yhi + 1.0);
    float a = h11(k*5.1 + fi*1.7);
    vec3 c0 = around(0.3, a, ya), c1 = around(0.3, a + 0.15*(h11(k*7.0 + fi) - 0.5), yb);
    vec3 mid = around(0.9 + 0.8*h11(k*9.3 + fi), a + 0.05, mix(ya, yb, 0.5));
    float d = min(bolt3(ro, rd, c0, mid, k*1.91 + fi*1.37 + floor(t*24.0)*0.13, 0.45, px),
                  bolt3(ro, rd, mid, c1, k*2.71 + fi*0.77 + floor(t*24.0)*0.29, 0.45, px));
    float flick = step(0.25, h11(floor(t*30.0) + fi*7.7));
    float life = sin(3.1416*q)*flick*part(0.0);
    red += (vec3(1.0, 0.92, 0.95)*halo(d, max(px, 0.03))*2.6 + vec3(1.0, 0.25, 0.3)*halo(d, 0.12)*1.6
            + BK_RED*halo(d, 0.5)*0.55 + BK_RED*halo(d, 1.4)*0.12)*life;
  }
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float rate = 0.5 + 0.3*h11(fi*2.7);
    float cyc = t*rate + h11(fi*3.1)*5.0;
    float k = floor(cyc), q = fract(cyc);
    if (q > 0.35) continue;
    float y = mix(ylo, yhi, h11(k*2.3 + fi));
    float a = h11(k*5.1 + fi*1.7);
    float reach = 4.5 + 2.5*h11(k + fi*4.0);
    vec3 a0 = around(0.35, a, y);
    vec3 a1 = around(0.35 + reach, a + 0.1*(h11(k*7.0 + fi) - 0.5), y + (h11(k*3.3 + fi) - 0.3)*reach);
    if (rayPt(ro, rd, (a0 + a1)*0.5, ta) > reach + 3.0) continue;
    float life = q/0.35;
    float grow = smoothstep(0.0, 0.15, life);
    float fade = 1.0 - smoothstep(0.5, 1.0, life);
    float flick = 0.55 + 0.45*step(0.35, h11(floor(t*30.0) + fi*7.7));
    float d = bk_bolt(ro, rd, a0, mix(a0, a1, grow), k*1.91 + fi*1.37 + floor(t*12.0)*0.21, 0.3);
    red += (vec3(1.0, 0.92, 0.95)*halo(d, max(px, 0.04))*2.4 + vec3(1.0, 0.25, 0.3)*halo(d, 0.16)*1.4
            + BK_RED*halo(d, 0.7)*0.45 + BK_RED*halo(d, 1.8)*0.12)*fade*flick*part(0.0);
    red += (vec3(1.0, 0.7, 0.75)*halo(rayPt(ro, rd, a0, ta), 0.4)*1.4)*(1.0 - smoothstep(0.0, 0.3, life))*part(0.0);
  }
  // ---- drops of red light bleeding off the corrupted part (round 15, Creyon: the sparks
  // looked cheap) ----
  // each drop swells on the blade, clinging, then lets go and falls faster and faster, drawn
  // as a teardrop streak stretched by its speed: a hot pink head, a red tail thinning behind it
  for (int i = 0; i < 12; i++){
    float fi = float(i);
    float rate = 0.32 + 0.18*h11(fi*2.9);
    float cyc = t*rate + h11(fi*5.3);
    float k = floor(cyc), f = fract(cyc);
    float y = mix(1.5, max(lv - 0.5, 2.5), h11(k*1.9 + fi));
    float a = h11(k*4.1 + fi);
    float cling = 0.35;
    float g = max(f - cling, 0.0)/(1.0 - cling);
    float fall = 9.0*g*g;
    float out = 0.45 + 0.5*g;
    vec3 head = around(out, a, y - fall);
    // the streak's length follows the speed (none while it clings)
    vec3 tail = around(out - 0.1*g, a, y - fall + 0.25 + 2.2*g);
    float sd;
    float d = raySeg(ro, rd, head, tail, sd);
    float swell = smoothstep(0.0, cling, f);
    float w = mix(0.17, 0.035, sd)*(0.6 + 0.4*swell);
    float life = swell*(1.0 - smoothstep(0.75, 1.0, f));
    red += (vec3(1.0, 0.6, 0.65)*halo(d, max(w*0.6, px))*1.6*(1.0 - sd) + BK_RED*halo(d, max(w*2.2, px))*1.8
            + BK_RED*halo(d, 0.45)*0.18)*life*part(1.0);
  }
  // ---- corrupted embers: from the red front where it fights the light, glowing shards of
  // kyber crack off in short hot arcs that curve down and die out ----
  for (int i = 0; i < 10; i++){
    float fi = float(i);
    float cyc = t*(0.55 + 0.3*h11(fi*1.7)) + h11(fi*8.1);
    float k = floor(cyc), q = fract(cyc);
    float a = h11(k*6.7 + fi*1.3);
    float y = lv + (h11(k*2.2 + fi) - 0.6)*2.0;
    vec3 dir = vec3(sin(6.2832*a), 0.0, cos(6.2832*a));
    float sp = 2.2 + 2.0*h11(k*3.9 + fi);
    // three points of its curved path: thrown out and up, then pulled down
    float q0 = max(q - 0.12, 0.0);
    vec3 p1 = vec3(0.0, y, 0.0) + dir*(0.4 + sp*q) + vec3(0.0, 2.2*q - 4.5*q*q, 0.0);
    vec3 p0 = vec3(0.0, y, 0.0) + dir*(0.4 + sp*q0) + vec3(0.0, 2.2*q0 - 4.5*q0*q0, 0.0);
    float se;
    float d = raySeg(ro, rd, p0, p1, se);
    float life = (1.0 - smoothstep(0.45, 1.0, q))*smoothstep(0.0, 0.04, q);
    vec3 hot = mix(vec3(1.0, 0.85, 0.85), BK_RED, smoothstep(0.1, 0.6, q));
    red += (hot*halo(d, max(0.035, px))*2.2*se + BK_RED*halo(d, 0.2)*0.5)*life*part(0.0);
  }
  // ---- where the white holds: beams of pure light pushing back, and its motes ----
  float ya = min(lv + 2.0, y1 - 2.0);
  col += fxMotes(ro, rd, px, ya, y1, 1.0, 5.0, 30.0, 0.07, 3.6, BK_WHITE)*part(2.0);
  col += fxAura(ro, rd, ya, y1, 3.0, BK_WHITE)*0.2*part(2.0);
  // round 16 (Creyon: the pure part needs more): beams of pure light flash out of it and push
  // back, and where red meets white the two fight in a spray of sparks of both colours
  col += fxRays(ro, rd, px, ya, y1, 6.0, 3.2, 0.9, 7.7, BK_WHITE)*0.55*part(2.0);
  col += fxSparks(ro, rd, px, lv - 0.8, lv + 0.8, 10.0, 2.6, 1.6, 2.9, BK_WHITE)*part(2.0);
  red += fxSparks(ro, rd, px, lv - 0.8, lv + 0.8, 8.0, 2.2, 1.4, 5.3, BK_RED)*part(0.0);
  float dfront = rayPtRaw(ro, rd, vec3(0.0, lv, 0.0), ta);
  col += BK_WHITE*halo(dfront, 0.9)*0.3*part(2.0);
  // the front: a ring of struggle where red meets white
  // the aura: red round the bleeding part, white above
  float dlo = fxNear(ro, rd, 0.0, lv), dhi = fxNear(ro, rd, lv, L);
  red += BK_RED*halo(dlo, 2.5)*0.3;
  col += BK_WHITE*halo(dhi, 3.0)*0.1;
  // the corruption stays red whatever colour the blade is worn in (uKeep, Creyon's round 12)
  uKeep += red*uFx;
  return col*uFx;
}

// the blade: Sol's glow and core from the file, white, bleeding red from the hilt with cracks
// glowing through the corruption
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float l = max(g.r, max(g.g, g.b));
  float m = bk_mask(b.y);
  vec2 cell;
  float e = voro(vec2(b.x*3.0, b.y*0.8), cell);
  float crack = halo(e, 0.06)*m;
  // round 12 (Creyon): the pure part takes its wearer's colour, the corruption stays red
  // round 15 (Creyon: some blades glowed so much their effects hardly showed): 70 %
  uKeep += (l*BK_RED*1.3*m*0.9 + BK_RED*crack*l*1.5)*0.9;
  // round 18 (Creyon: the pure light grew over the red part): the pure part at half, the
  // corruption stronger
  return l*BK_WHITE*(1.0 - 0.9*m)*0.45;
}
vec3 sig_core(vec3 c, vec2 b, float px){
  float m = bk_mask(b.y);
  return c*(1.0 - 0.15*m);
}
