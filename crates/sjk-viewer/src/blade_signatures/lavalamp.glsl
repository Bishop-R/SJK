// Coaxium: a blade of raw coaxium, the volatile hyperfuel of Kessel. Glowing violet-red globules
// of the fuel rise and churn inside the blade, merging and splitting; it is never quite stable:
// it shivers, flickers hot, and spits bright unstable sparks and short arcs off its surface,
// with a pale vapour boiling off it. A swing makes it churn and spark; a clash nearly sets it
// off in a blinding flash and a ring of sparks.
// @part Coaxium sparks
// @room 6
// @palette {"corona.rim_cool":[0.35,0.04,0.30],"corona.rim_hot":[1.0,0.25,0.55],"corona.inner":[1.0,0.75,0.88],"core.white":[1.0,0.92,0.96],"core.fringe_cool":[0.85,0.15,0.55],"core.fringe_hot":[1.0,0.55,0.75],"light.color":[1.0,0.30,0.60],"trail":[1.0,0.35,0.65]}

float cx_top(){ return uLength*clamp(uAge/0.45, 0.0, 1.0); }
// how unstable it is now: flickers up now and then
float cx_unst(){
  float k = floor(uTime*3.0);
  return 0.3 + 0.7*step(0.7, h11(k*1.7))*exp(-fract(uTime*3.0)*3.0) + 0.6*clamp(uSwing, 0.0, 1.0);
}
vec3 base_vol(vec3 ro, vec3 rd, float px){
  float t = uTime, top = cx_top(), s, ta;
  float cl = clamp(uClash, 0.0, 1.0);
  vec3 hot = vec3(1.0, 0.3, 0.6), white = vec3(1.0, 0.9, 0.95);
  vec3 col = vec3(0.0);
  if (top < 0.5) return col;
  float un = cx_unst();
  // unstable sparks spat off the surface
  for (int i = 0; i < 18; i++){
    float fi = float(i);
    float f = fract(t*(0.9 + 0.6*h11(fi*2.1)) + h11(fi*4.7));
    float k = floor(t*(0.9 + 0.6*h11(fi*2.1)) + h11(fi*4.7));
    if (h11(k*3.1 + fi) > 0.35 + 0.5*un) continue;
    float y0 = (0.05 + 0.9*h11(fi*6.3 + k))*top;
    vec3 dir = normalize(vec3(sin(6.2832*h11(fi*1.9 + k)), 0.5*(h11(fi*8.3 + k) - 0.3), cos(6.2832*h11(fi*1.9 + k))));
    vec3 p0 = vec3(0.0, y0, 0.0) + dir*(0.4 + 3.5*f);
    vec3 p1 = p0 - dir*(0.25 + 0.5*(1.0 - f));
    float d = raySeg(ro, rd, p1, p0, s);
    col += mix(white, hot, f)*(halo(d, max(0.04, px))*1.6 + halo(d, 0.25)*0.2)*(1.0 - f)*s;
  }
  // short unstable arcs crackling over it
  for (int i = 0; i < 3; i++){
    float fi = float(i);
    float k = floor(t*8.0 + fi*0.3);
    if (h11(k*2.7 + fi) > 0.25 + 0.5*un) continue;
    float y0 = (0.1 + 0.8*h11(k*1.3 + fi))*top;
    float a = h11(k*4.1 + fi);
    float bd = bolt3(ro, rd, around(0.4, a, y0), around(0.4 + 1.2*h11(k + fi), a + 0.15, y0 + 1.5 + 1.5*h11(k*5.0)), k + fi*9.0, 0.45, px);
    col += boltLight(bd, hot, px)*0.9;
  }
  // vapour boiling off it
  for (int i = 0; i < 8; i++){
    float fi = float(i);
    float f = fract(t*0.3 + fi/8.0);
    vec3 p = around(0.6 + 1.0*f, fi*0.41 + 0.2*f, (0.1 + 0.8*h11(fi*3.3 + floor(t*0.3 + fi/8.0)))*top + 2.0*f);
    float d = rayPt(ro, rd, p, ta);
    col += vec3(0.9, 0.6, 0.8)*halo(d, 0.4 + 0.5*f)*0.15*sin(3.1416*f);
  }
  // a clash: a blinding flash and a ring of sparks
  if (cl > 0.01){
    float yc = uLength*0.6;
    float dc = rayPtRaw(ro, rd, vec3(0.0, yc, 0.0), ta);
    col += white*(halo(dc, 1.5)*2.5 + halo(dc, 3.5)*0.6)*cl*cl;
    for (int i = 0; i < 14; i++){
      float fi = float(i);
      vec3 dir = vec3(sin(fi*0.449), 0.2*(h11(fi) - 0.5), cos(fi*0.449));
      vec3 p0 = vec3(0.0, yc, 0.0) + dir*(0.5 + 5.0*(1.0 - cl));
      float d = raySeg(ro, rd, p0 - dir*0.6, p0, s);
      col += hot*halo(d, max(0.05, px))*1.8*cl;
    }
  }
  return col*uFx;
}
// the blade: Sol's glow and core from the file with globules of coaxium rising and churning in it
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float t = uTime;
  float blob = 0.0;
  for (int i = 0; i < 6; i++){
    float fi = float(i);
    float y = mod(t*(1.5 + 0.8*h11(fi*3.1)) + fi*7.0, uLength + 4.0) - 2.0;
    float x = 0.25*sin(t*0.9 + fi*2.0);
    float R = 0.35 + 0.2*sin(t*1.3 + fi);
    blob = max(blob, smoothstep(R*1.6, R*0.4, length(vec2((b.x - x)*1.6, (b.y - y)*0.45))));
  }
  float sh = 1.0 + 0.15*sin(t*31.0 + b.y)*cx_unst();
  return g*(0.75 + 0.9*blob + 0.25*cx_unst() + 1.5*clamp(uClash, 0.0, 1.0)*clamp(uClash, 0.0, 1.0))*sh;
}
vec3 sig_core(vec3 c, vec2 b, float px){ return c*(1.0 + 0.2*sin(uTime*29.0)*cx_unst()); }
// round 11: sig_vol gated by part 0
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  if (part(0.0) < 0.5) { return vec3(0.0); }
  return base_vol(ro, rd, px);
}
