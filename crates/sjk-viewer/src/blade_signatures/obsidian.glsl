// Obsidian Edge: Creyon's Blade Lab design, reworked in full. A flat blade of black volcanic glass,
// as thin as a stock blade, with glassy facets catching the light, white-hot edges crackling along
// both sides and lava veins pulsing up through the glass. Round it, in 3D, a ring of obsidian
// shards orbits slowly, black glass with glowing magma edges, magma lightning leaps between the
// blade and the shards, white-hot beams flash off the edge and the air shimmers with heat. A
// swing spins the shards out wider; a clash shatters a burst of shards off the blade with a white
// flash and a shock ring; ignition: the glass is forged hot from the hilt.
// @full 1
// @part Heat glow
// @part Obsidian shards
// @room 10
// @solid 1

float ob_w(float y){                      // half width of the flat glass along the blade
  float L = uLength;
  return 0.56*(1.0 - smoothstep(L - 2.2, L + 0.05, y)) + 0.05;
}
// the edge's crackle: bright flickering segments
float ob_crack(float y, float t){
  float n = vn(vec2(y*1.7, t*7.0))*0.6 + vn(vec2(y*5.0, t*13.0))*0.4;
  return 0.55 + 0.9*smoothstep(0.45, 0.85, n);
}
// lava veins: thin branching cracks drifting up through the glass
float ob_vein(vec2 b, float t){
  vec2 q = vec2(b.x*3.2, b.y*0.55 - t*0.9);
  float n = vn(q*1.7);
  float line = abs(n - 0.5);
  return smoothstep(0.06, 0.0, line)*smoothstep(0.35, 0.7, vn(vec2(b.y*0.25 - t*0.6, 3.0)));
}
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float L = uLength;
  px = min(px, 0.06);
  float on = smoothstep(-0.6, 0.3, b.y);
  float edge = 1.0 - smoothstep(6.5, 8.5, abs(b.x));
  float forge = 1.0 - smoothstep(0.0, 1.8, uAge);                    // ignition heat
  float w = ob_w(b.y);
  float ax = abs(b.x);
  // round 23 (Creyon's game, 22/10/2026: an orange box ended at a straight line level with the
  // tip): the edges' blooms fade out round the tip and into the hilt instead of stopping at once
  float inside = (1.0 - smoothstep(L - 0.6, L + 1.6, b.y))*smoothstep(-0.8, -0.1, b.y);
  // white-hot edges: thin bright lines on both sides, crackling, with a hot orange bloom
  float de = abs(ax - w);
  float cr = ob_crack(b.y + sign(b.x)*7.0, uTime);
  vec3 light = vec3(1.0, 0.97, 0.92)*exp(-pow(de/0.07, 2.0))*1.6*cr*inside;
  light += vec3(1.0, 0.5, 0.15)*halo(de, 0.35)*0.5*cr*inside + vec3(1.0, 0.35, 0.08)*halo(de, 1.4)*0.12*inside;
  // the tip: the two edges meet in a hot point
  float dt = length(vec2(b.x, max(b.y - L, 0.0)));
  light += vec3(1.0, 0.8, 0.5)*halo(dt, 0.25)*0.5*smoothstep(L - 1.2, L - 0.3, b.y);
  // sparks running along the edge
  for (int k = 0; k < 5; k++){
    float fk = float(k);
    float y = mod(uTime*(9.0 + 3.0*fk) + h11(fk*4.0)*L, L + 4.0) - 2.0;
    float s = mod(fk, 2.0) < 0.5 ? 1.0 : -1.0;
    light += vec3(1.0, 0.9, 0.7)*halo(length(vec2(b.x - s*ob_w(y), b.y - y)), 0.12)*1.5;
  }
  // clash: a burst of big shards blown off the strike point with a white flash
  if (uClash > 0.01){
    float a = 1.0 - uClash;
    vec2 o = vec2(0.0, L*0.55);
    light += vec3(1.0, 0.95, 0.85)*halo(length(b - o), 1.2)*pow(uClash, 4.0)*1.5;
    for (int k = 0; k < 10; k++){
      float fk = float(k);
      float an = fk*0.628 + 0.4*h11(fk*2.0);
      vec2 dir = vec2(cos(an), sin(an));
      vec2 p = o + dir*(0.5 + 7.0*(1.0 - exp(-3.0*a)));
      vec2 q = b - p;
      float rr = uTime*6.0 + fk;
      q = vec2(q.x*cos(rr) - q.y*sin(rr), q.x*sin(rr) + q.y*cos(rr));
      float sh = max(abs(q.x) - 0.12, abs(q.y) - 0.38 + abs(q.x)*1.6);
      float heat = uClash;
      light += mix(vec3(0.6, 0.1, 0.03), vec3(1.0, 0.85, 0.6), heat)*(fillD(sh, px) + halo(max(sh, 0.0), 0.25)*0.5);
    }
  }
  light *= on*edge*(1.0 - smoothstep(L + 1.5, L + 5.0, b.y))*smoothstep(-3.0, -0.5, b.y);
  return (1.0 - exp(-light*1.15))*1.05*clamp(uIgn*1.5, 0.0, 1.0);
}
vec3 sig_core(vec3 c, vec2 b, float px){ return vec3(0.0); }
// the glass itself: black, faceted, with moving glints and the lava veins inside
vec4 sig_solid(vec4 o, vec2 b, float px){
  px = min(px, 0.06);
  float L = uLength;
  float w = ob_w(b.y);
  float cov = fillD(abs(b.x) - w, px)*step(-0.3, b.y)*fillD(b.y - L - 0.05, px);
  if (cov <= 0.0) return o;
  float forge = 1.0 - smoothstep(0.0, 1.8, uAge);
  // facets: conchoidal ripples in the glass, a ridge down the middle
  float ridge = 1.0 - abs(b.x)/max(w, 0.01);
  float conch = vn(vec2(b.x*4.0, b.y*0.9))*0.6 + vn(vec2(b.x*9.0, b.y*2.3))*0.4;
  float rings = 0.5 + 0.5*sin(conch*14.0);
  // a glint sweeping up the blade, and a cool sky sheen on one face
  float gl = exp(-pow((b.y - mod(uTime*11.0, L + 16.0) + 8.0 + b.x*3.0)/1.2, 2.0));
  vec3 glass = vec3(0.025, 0.022, 0.03) + vec3(0.06, 0.07, 0.1)*rings*0.6 + vec3(0.12, 0.13, 0.17)*smoothstep(0.7, 1.0, ridge)*0.6;
  glass += vec3(0.75, 0.8, 0.95)*gl*(0.25 + 0.5*rings)*smoothstep(0.1, 0.9, ridge);
  // lava veins glowing inside, brighter while forging and on swing
  float vein = ob_vein(b, uTime);
  vec3 lava = mix(vec3(0.9, 0.2, 0.03), vec3(1.0, 0.75, 0.3), vein);
  glass = mix(glass, lava*(1.2 + 0.8*uSwing), vein*(0.7 + 0.3*sin(uTime*3.0 + b.y*0.4)));
  glass = mix(glass, vec3(1.0, 0.45, 0.1), forge*0.85);
  return vec4(mix(o.rgb, glass, cov), max(o.a, cov*0.98*clamp(uIgn*1.5, 0.0, 1.0)));
}

// ---- round 9 (Creyon): no flying chips; obsidian shards orbiting in 3D, magma arcs, beams ----
// a shard of black glass at c: how much of the ray it covers, where (th) and its shading
// (sh.x facet light, sh.y the glowing edge)
float ob_shard(vec3 ro, vec3 rd, vec3 c, float sz, float spin, float seed, float px, out float th, out vec2 sh){
  float d = rayPt(ro, rd, c, th);
  sh = vec2(0.0);
  if (d > sz*2.0 + px) return 0.0;
  vec3 o = ro + rd*th - c;
  vec2 a = rot(spin + seed*6.0)*o.xy; o = vec3(a, o.z);
  vec2 b = rot(spin*0.7 + seed*3.0)*o.yz; o = vec3(o.x, b);
  // a long sharp sliver of glass, pointed at both ends
  float k = max(abs(o.x)*1.8 + abs(o.y)*0.45, abs(o.z)*3.0) - sz*0.7;
  sh = vec2(0.5 + 0.5*h21(sign(o.xy) + vec2(seed*7.0, sign(o.z))), halo(abs(k), 0.04 + px*0.8));
  return clamp(0.5 - k/max(px, 1e-3), 0.0, 1.0);
}
// shard i's place round the blade
vec3 ob_place(float fi){
  float L = uLength, sw = clamp(uSwing, 0.0, 1.0);
  float a = h11(fi*1.7) + uTime*(0.05 + 0.04*h11(fi*2.9));
  float R = 2.4 + 2.2*h11(fi*5.1) + 2.0*sw;
  float y = (0.18 + 0.78*fract(fi*0.381 + 0.07))*L + 0.8*sin(uTime*0.6 + fi*2.0);
  return around(R, a, y);
}
// round 10: the shards circle the blade, so they are drawn in true 3D (only orbits are)
vec4 base_orbit_solid(vec3 ro, vec3 rd, float px){
  vec4 col = vec4(0.0);
  if (uFx < 0.01 || fxNear(ro, rd, uLength*0.1, uLength) > 8.0) return col;
  float bestT = 1e5;
  for (int i = 0; i < 18; i++){
    float fi = float(i);
    float th; vec2 sh;
    float cv = ob_shard(ro, rd, ob_place(fi), 1.2 + 0.7*h11(fi*9.1), uTime*(0.4 + 0.5*h11(fi)) + fi, fi, px, th, sh);
    if (cv > 0.0 && th < bestT){
      bestT = th;
      float pulse = 0.6 + 0.4*sin(uTime*2.0 + fi*1.3);
      vec3 rgb = vec3(0.03, 0.028, 0.035) + vec3(0.25, 0.27, 0.33)*pow(sh.x, 3.0)*0.6 + vec3(1.0, 0.45, 0.1)*sh.y*1.6*pulse;
      col = vec4(rgb, cv);
    }
  }
  return col;
}
vec3 base_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 hot = vec3(1.0, 0.45, 0.1), white = vec3(1.0, 0.92, 0.8);
  vec3 col = vec3(0.0);
  if (fxNear(ro, rd, y0, y1) > 10.0) return col;
  // white-hot beams flashing off the edges
  col += fxAura(ro, rd, y0, y1, 2.8, hot)*0.8;
  // a clash: a shock ring and a flash
  return col*uFx;
}
// the shards' glow and the magma lightning leaping from the blade to them, in true 3D with them
vec3 base_orbit(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0);
  vec3 hot = vec3(1.0, 0.45, 0.1);
  vec3 col = vec3(0.0);
  if (fxNear(ro, rd, L*0.1, L) > 10.0) return col;
  float ta;
  // the shards' glow, and magma lightning leaping from the blade to them
  for (int i = 0; i < 18; i++){
    float fi = float(i);
    vec3 p = ob_place(fi);
    col += hot*halo(rayPt(ro, rd, p, ta), 1.0)*0.12;
    float k = floor(t*1.3 + h11(fi*3.3)*7.0), q = fract(t*1.3 + h11(fi*3.3)*7.0);
    if (h11(k*2.7 + fi) > 0.35 + 0.4*sw) continue;
    float d = bolt3(ro, rd, vec3(0.0, p.y + (h11(k + fi) - 0.5)*3.0, 0.0), p, k + fi*1.7, 0.45, px);
    col += boltLight(d, hot, px)*(1.0 - q)*(1.0 - q);
  }
  return col*uFx;
}
// round 11: sig_vol gated by part 0
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  if (part(0.0) < 0.5) { return vec3(0.0); }
  return base_vol(ro, rd, px);
}
// round 11: sig_orbit gated by part 1
vec3 sig_orbit(vec3 ro, vec3 rd, float px){
  if (part(1.0) < 0.5) { return vec3(0.0); }
  return base_orbit(ro, rd, px);
}
// round 11: sig_orbit_solid gated by part 1
vec4 sig_orbit_solid(vec3 ro, vec3 rd, float px){
  if (part(1.0) < 0.5) { return vec4(0.0); }
  return base_orbit_solid(ro, rd, px);
}
