// Starforge: a kyber blade being forged inside a star. A molten white-gold core burns under a living
// plasma surface that boils like the face of a sun, and tongues of solar fire lick up off it, as flares
// arch out round the blade and fall back. In a steady rhythm the forge strikes: a white flash where
// the blow lands, a hot spot that cools back to gold, bright light racing up and down the blade
// and a burst of sparks flying out on every side. Embers drift all round inside a wide warm heat glow.
// A swing makes it burn brighter and throw more sparks (the rhythm never speeds up); a clash is a huge
// blow with a ring of sparks. Round 19 (Creyon: it did not feel mythical): the forge draws the
// star itself down into it, two streams of plasma spiralling down a funnel round the blade with
// knots of light pouring into the strike, and every fourth blow a forge nova bursts out, a shell
// of gold light racing out round it in 3D with beams and the blade flaring white.
// @part The forge
// @part Star siphon
// @part Forge nova
// @motes Rising sparks
// @room 10
// @palette {"corona.rim_cool":[0.55,0.12,0.02],"corona.rim_hot":[1.0,0.55,0.12],"corona.inner":[1.0,0.88,0.55],"core.white":[1.0,0.96,0.85],"core.fringe_cool":[1.0,0.40,0.08],"core.fringe_hot":[1.0,0.80,0.45],"light.color":[1.0,0.58,0.18],"trail":[1.0,0.55,0.15]}


// one hammer blow every SF_P seconds, always (a swing never changes the rate)
const float SF_P = 1.25;
float sf_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
// star colour by temperature: deep red, orange, gold, white
vec3 sf_heat(float k){
  vec3 c = mix(vec3(0.45, 0.05, 0.0), vec3(1.0, 0.38, 0.05), smoothstep(0.0, 0.45, k));
  c = mix(c, vec3(1.0, 0.75, 0.32), smoothstep(0.4, 0.85, k));
  return mix(c, vec3(1.0, 0.96, 0.86), smoothstep(0.85, 1.25, k));
}
// the ray's distance to a ring of radius R round the blade at height y: the nearer of where the
// ray crosses the ring's plane and where it crosses the ring's cylinder (smooth from any side)
float sf_ring(vec3 ro, vec3 rd, float y, float R){
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
// a blow's height and side
float sf_y(float n){ return uLength*(0.18 + 0.68*h11(n*3.71 + 0.37)); }

vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, top = sf_top();
  float st = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float fx = uFx;
  vec3 white = vec3(1.0, 0.96, 0.86), gold = vec3(1.0, 0.72, 0.28), hot = vec3(1.0, 0.42, 0.07);
  float n = floor(t/SF_P), ph = fract(t/SF_P)*SF_P;
  float ys = sf_y(n), yp = sf_y(n - 1.0);
  float as = h11(n*1.37 + 0.2);

  // The blade itself is Sol's glow and core from the skin's file (sig_glow, sig_core);
  // everything here is a piece placed round it in 3D, each one facing the camera.
  float s;
  vec3 col = vec3(0.0);
  if (top < 0.2) return col;

  // ---- the plasma surface: granules of star-stuff boiling up round the blade ----
  float ta;
  // 31/10/2026: the granules keep within 0.9 of the axis from just under the hilt to the top,
  // their widest glow 0.6 (and a pixel's): a ray further off than three of them skips them
  if (raySegRaw(ro, rd, vec3(0.0, -0.3, 0.0), vec3(0.0, top, 0.0), s) - 0.9 <= 1.8 + 3.0*px)
  for (int i = 0; i < 26; i++){
    float fi = float(i);
    float f = fract(t*(0.22 + 0.12*h11(fi*1.7)) + h11(fi*4.3));
    float yy = -0.3 + f*(top + 0.3);
    vec3 p = around(0.55 + 0.35*h11(fi*2.9 + floor(t*0.22 + h11(fi*4.3))), h11(fi*6.1) + t*0.05, yy);
    float dd = rayPt(ro, rd, p, ta);
    float life = sin(3.1416*f);
    float spt = exp(-pow((yy - ys)/2.0, 2.0))*exp(-ph*1.8);
    float g = 0.5 + 0.5*sin(t*6.0 + fi*2.7);
    col += sf_heat(0.55 + 0.4*g + spt)*(halo(dd, max(0.16 + 0.1*g, px))*1.1 + halo(dd, 0.6)*0.2)*life*(1.0 + 0.5*st + spt)*fx;
  }

  // the rest is out of the hilt's way: skip rays far from the blade
  float far = raySegRaw(ro, rd, vec3(0.0, -1.0, 0.0), vec3(0.0, top + 1.0, 0.0), s);
  if (far > 9.0) return col;

  // ---- solar flares: tongues of plasma licking up off the blade (round 13, Creyon: no loops
  // round the blade; they were plasma loops arching out and back) ----
  // round 15 (Creyon: it looked like the Phoenix): a star's white-gold plasma, not orange flame
  col += fxSmoke(ro, rd, 0.15*L, top, 1.6, 6.0, 2.4, vec3(1.0, 0.72, 0.3))*1.1*fx;
  col += fxSmoke(ro, rd, 0.15*L, top, 0.9, 8.0, 5.9, vec3(1.0, 0.93, 0.75))*0.8*fx;
  for (int i = 0; i < 0; i++){
    float fi = float(i);
    float cyc = t/3.6 + fi*0.173 + h11(fi*5.3)*0.5;
    float k = floor(cyc), q = fract(cyc);
    float y0 = (0.08 + 0.78*h11(k*2.13 + fi*7.7))*L;
    float dy = 4.0 + 4.0*h11(k*3.9 + fi);
    if (y0 + dy > top) continue;
    float H = (3.2 + 3.0*h11(k*1.7 + fi*2.0))*smoothstep(0.0, 0.45, q);
    float a = h11(k*6.1 + fi*1.3);
    float env = smoothstep(0.0, 0.12, q)*(1.0 - smoothstep(0.65, 1.0, q));
    if (far > H + 3.0) continue;
    float best = 1e3, bs = 0.0;
    vec3 prev = around(0.4, a, y0);
    for (int j = 1; j <= 8; j++){
      float f = float(j)/8.0;
      vec3 qq = around(0.4 + H*sin(3.1416*f), a + 0.06*sin(3.1416*f) + 0.02*sin(t*1.3 + fi), y0 + dy*f);
      float dd = raySeg(ro, rd, prev, qq, s);
      if (dd < best){ best = dd; bs = (float(j) - 1.0 + s)/8.0; }
      prev = qq;
    }
    float foot = 1.0 - 0.5*sin(3.1416*bs);
    float fl = 0.45 + 1.0*vn(vec2(bs*9.0 - t*2.5, fi*3.0 + k));
    col += (sf_heat(0.7 + 0.45*fl)*halo(best, max(0.07 + 0.16*fl, px))*1.3*fl + hot*halo(best, 0.7)*0.6 + vec3(1.0, 0.25, 0.03)*halo(best, 1.4)*0.16)*env*foot*fx;
  }

  // ---- the hammer blow: flash, shock rings, sparks ----
  if (ys < top){
    vec3 o = around(0.45, as, ys);
    float dp = rayPt(ro, rd, o, ta);
    float k = exp(-ph*7.0);
    col += white*(halo(dp, 0.4 + 0.9*k)*2.2 + exp(-dp/2.5)*0.5)*k*fx;
    // round 13 (Creyon): no rings round the blade; the strike's light runs up and down it
    float fade = exp(-ph*2.2)*smoothstep(0.0, 0.04, ph);
    float yu = ys + 20.0*ph, yd = ys - 13.0*ph;
    if (yu < top){ float du = rayPtRaw(ro, rd, vec3(0.0, yu, 0.0), ta); col += (white*halo(du, 0.5)*1.2 + gold*halo(du, 1.2)*0.4)*fade*fx; }
    if (yd > 0.0){ float dr = rayPtRaw(ro, rd, vec3(0.0, yd, 0.0), ta); col += (white*halo(dr, 0.5)*1.2 + gold*halo(dr, 1.2)*0.4)*fade*fx; }
    // sparks flying out in all directions
    float dist0 = rayPtRaw(ro, rd, vec3(0.0, ys, 0.0), ta);
    if (dist0 < 9.0){
      for (int i = 0; i < 22; i++){
        float fi = float(i);
        float sa = as + (h11(fi*3.1 + n) - 0.5)*0.9;
        float up = (h11(fi*7.3 + n*1.3) - 0.35)*1.2;
        vec3 dir = normalize(vec3(sin(6.2832*sa), up, cos(6.2832*sa)));
        float v = 9.0 + 9.0*h11(fi*1.7 + n*0.7);
        float tp = ph, tq = max(ph - 0.05, 0.0);
        vec3 p1 = o + dir*v*(1.0 - exp(-tp*2.0))/2.0 + vec3(0.0, 1.5*tp*tp, 0.0);
        vec3 p0 = o + dir*v*(1.0 - exp(-tq*2.0))/2.0 + vec3(0.0, 1.5*tq*tq, 0.0);
        float dd = raySeg(ro, rd, p0, p1, s);
        float life = (1.0 - smoothstep(0.35, 1.1, ph))*(0.7 + 0.6*st);
        col += mix(white, hot, smoothstep(0.0, 0.8, ph))*(halo(dd, max(0.06, px))*2.4 + halo(dd, 0.3)*0.45)*life*fx;
      }
    }
  }

  // ---- embers drifting round in the heat ----
  // 31/10/2026: within 4.3 of the axis, from under the hilt to past the top, their widest glow
  // 0.3 (and a pixel's)
  if (fxNear(ro, rd, -1.0, top + 2.0) - 4.3 <= 0.9 + 3.0*px)
  for (int i = 0; i < 12; i++){
    float fi = float(i);
    float f = fract(t*(0.05 + 0.03*h11(fi*2.9)) + h11(fi*5.1));
    float yy = f*(top + 3.0) - 1.0;
    vec3 p = around(1.3 + 3.0*h11(fi*3.7), h11(fi*1.9) + t*0.03 + 0.1*sin(t*0.7 + fi), yy);
    float dd = rayPt(ro, rd, p, ta);
    float tw = 0.6 + 0.4*sin(t*5.0 + fi*2.3);
    col += hot*(halo(dd, max(0.06, px))*1.6 + halo(dd, 0.3)*0.15)*tw*smoothstep(0.0, 0.1, f)*(1.0 - smoothstep(0.75, 1.0, f))*(1.0 + st)*fx;
  }

  // ---- clash: a huge blow and a ring of sparks ----
  if (cl > 0.01){
    float a = 1.0 - cl;
    float yc = min(L*0.6, top);
    float dc = rayPtRaw(ro, rd, vec3(0.0, yc, 0.0), ta);
    col += white*(halo(dc, 1.5)*2.5 + halo(dc, 3.2)*0.7)*cl*cl;
    float dr = sf_ring(ro, rd, yc, 0.6 + 7.0*(1.0 - exp(-3.0*a)));
    col += mix(white, hot, a)*(halo(dr, max(0.08, px))*1.8 + halo(dr, 0.5)*0.4)*cl;
  }
  return col;
}

// the blade: Sol's glow and core from the file, white-hot where the last blow landed
float sf_spot(float y){
  float n = floor(uTime/SF_P), ph = fract(uTime/SF_P)*SF_P;
  return exp(-pow((y - sf_y(n))/1.6, 2.0))*exp(-ph*1.8) + 0.25*clamp(uClash, 0.0, 1.0);
}
vec3 sig_glow_own(vec3 g, vec2 b, float r, float px){
  float nova = exp(-sf_nova()*9.0)*part(2.0);
  return g*(1.0 + 0.25*sin(uTime*2.1 + b.y*0.4)*0.4 + 1.2*sf_spot(b.y) + 0.3*uSwing + 1.4*nova);
}
vec3 sig_core(vec3 c, vec2 b, float px){ return c*(1.0 + 0.8*sf_spot(b.y)); }

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
// round 19: the forge nova, every fourth blow: how far into it (0 to 1; 1 when none)
float sf_nova(){ float c = uTime/(SF_P*4.0); return fract(c); }
// round 19: the star siphon, two streams of plasma spiralling down a funnel into the forge
vec3 sf_siphon(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, top = sf_top(), s;
  vec3 acc = vec3(0.0);
  if (top < L*0.6 || fxNear(ro, rd, 0.0, L) > 5.5) return acc;
  for (int j = 0; j < 2; j++){
    float fj = float(j);
    float best = 1e3, bs = 0.0;
    vec3 prev = vec3(0.0);
    for (int i = 0; i <= 22; i++){
      float f = float(i)/22.0;
      float y = mix(L*0.93, L*0.12, f);
      float R = 0.5 + 2.9*pow(1.0 - f, 1.4);
      vec3 p = around(R, fj*0.5 + (1.0 - f)*1.5 + t*0.07, y);
      if (i > 0){
        float d = raySeg(ro, rd, prev, p, s);
        if (d < best){ best = d; bs = (float(i) - 1.0 + s)/22.0; }
      }
      prev = p;
    }
    float knot = pow(0.5 + 0.5*sin(bs*26.0 - t*5.0 + fj*3.0), 5.0);
    float ends = smoothstep(0.0, 0.12, bs)*(1.0 - smoothstep(0.88, 1.0, bs));
    vec3 c = sf_heat(1.15 - 0.55*bs);
    acc += (vec3(1.0, 0.97, 0.9)*halo(best, max(0.05, px))*(0.35 + 1.6*knot) + c*halo(best, 0.3)*(0.45 + 0.9*knot) + c*halo(best, 1.1)*0.1)*ends;
  }
  return acc;
}
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxAura(ro, rd, y0, y1, 3.6, vec3(1.0, 0.75, 0.35))*0.8*part(0.0);
  fx += sf_siphon(ro, rd, px)*0.9*part(1.0);
  // the forge nova: a shell of gold light racing out round the blade, beams through it
  float q = sf_nova();
  if (q < 0.6 && sf_top() > L*0.6){
    float qq = q/0.6;
    vec3 c = vec3(L*0.0, L*0.55, 0.0);
    float tq;
    float rad = 0.6 + 0.85*uReach*(1.0 - pow(1.0 - qq, 2.5));
    float dc = rayPt(ro, rd, c, tq);
    // the shell seen as a ring where the ray grazes it, and faintly across its face
    float shell = halo(abs(dc - rad), 0.35 + 0.4*qq)*1.3 + (1.0 - smoothstep(rad*0.6, rad, dc))*0.12;
    float env = (1.0 - qq)*(1.0 - qq)*smoothstep(0.0, 0.03, qq);
    fx += mix(vec3(1.0, 0.95, 0.82), vec3(1.0, 0.6, 0.15), qq)*shell*env*part(2.0);
    fx += fxRays(ro, rd, px, L*0.35, L*0.75, 14.0, 6.0, 1.0/(SF_P*4.0), 4.4, vec3(1.0, 0.8, 0.4))*env*1.2*part(2.0);
  }
  return col + fx*uFx;
}

// round 15 (Creyon: some blades glowed so much their effects hardly showed): its glow at
// 75 %, so what is drawn round it reads
vec3 sig_glow(vec3 g, vec2 b, float r, float px){ return sig_glow_own(g, b, r, px)*0.75; }
