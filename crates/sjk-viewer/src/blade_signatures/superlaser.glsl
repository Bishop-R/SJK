// Superlaser: a battle station's main weapon as a blade. Eight thin tributary beams rise from a
// ring round the emitter and lean in to meet at a blazing focus a hand's length above the hilt:
// a compact cone of beams that, seen from above, makes an eight-pointed star round the blade.
// Pulses of energy race up each tributary into the focus, which crackles with arcs. From the
// focus the main beam fires to the tip: a white-hot core in a green plasma sheath that ripples,
// with two plasma strands twisting round it and surges racing up. On ignition the tributaries
// light first, then the beam fires. A swing flares the tributaries and sends a surge; a clash
// fires the focus in a white flash and a shockwave (round 15: no lightning along it, which made
// it look like the Sith Alchemy and Witchfire blades).
// @part Superlaser beam
// @part Plasma sparks
// @room 10
// @palette {"corona.rim_cool":[0.05,0.55,0.12],"corona.rim_hot":[0.45,1.0,0.35],"corona.inner":[0.85,1.0,0.8],"core.white":[0.95,1.0,0.94],"core.fringe_cool":[0.2,0.9,0.3],"core.fringe_hot":[0.7,1.0,0.6],"light.color":[0.3,1.0,0.35],"trail":[0.35,1.0,0.4]}


const float SL_F = 5.8;    // the focus's height
const float SL_R = 2.2;    // the tributaries' ring radius at the base
const float SL_B = 0.4;    // the tributaries' base height

vec3 r8_vol(vec3 ro, vec3 rd, float px){
  float L = uLength, t = uTime;
  float sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  vec3 green = vec3(0.25, 1.0, 0.3), pale = vec3(0.8, 1.0, 0.75), white = vec3(1.0);
  float lit = smoothstep(0.0, 0.16, uAge);                       // the tributaries light first
  float fire = clamp((uAge - 0.1)/0.35, 0.0, 1.0);               // then the beam fires up
  float top = mix(SL_F, L, fire);
  vec3 F = vec3(0.0, SL_F, 0.0);
  vec3 light = vec3(0.0);
  float fx = uFx;
  // The beam is Sol's glow and core from the file with surges racing up it (sig_glow,
  // sig_core); here only the effects, each a piece placed round it in 3D.
  float s;
  if (fx > 0.0){
    vec3 fxl = vec3(0.0);
    float near = raySegRaw(ro, rd, vec3(0.0, 0.0, 0.0), vec3(0.0, top, 0.0), s);
    // ---- two plasma strands twisting round the main beam
    if (near < 2.5 && fire > 0.0){
      for (int h = 0; h < 2; h++){
        float fh = float(h);
        vec3 prev = F;
        for (int k = 1; k <= 20; k++){
          float f = float(k)/20.0;
          float y = mix(SL_F, top, f);
          float r = 0.9 + 0.25*sin(y*0.7 - t*6.0 + fh*3.0);
          vec3 q = around(r*smoothstep(0.0, 0.12, f), fh*0.5 + y*0.045 - t*1.3, y);
          float sq;
          float d = raySeg(ro, rd, prev, q, sq);
          float yy = mix(prev.y, q.y, sq);
          float pul = 0.6 + 0.4*sin(yy*0.9 - t*9.0) + 0.6*halo(abs(yy - fract(t*0.9)*(L + 4.0) + 2.0), 1.5);
          fxl += (white*halo(d, max(px, 0.04))*0.9 + green*halo(d, 0.2)*0.5)*pul*(1.0 - smoothstep(top - 2.0, top, yy));
          prev = q;
        }
      }
    }
    // ---- the tributaries: a compact cone of eight beams converging on the focus
    if (near < 4.5){
      float spin = t*0.02;
      for (int i = 0; i < 8; i++){
        float fi = float(i);
        vec3 a = around(SL_R, fi/8.0 + spin, SL_B);
        float u;
        float d = raySeg(ro, rd, a, F, u);
        if (d > 1.2) continue;
        float flick = 0.85 + 0.15*sin(t*40.0 + fi*2.1);
        // pulses racing up into the focus
        float ph = fract(u*1.5 - t*2.2 - fi*0.37);
        float pul = exp(-pow((ph - 0.5)/0.1, 2.0))*2.4;
        float bright = (0.55 + 0.45*u + pul)*flick*lit*(1.0 + 1.3*sw);
        fxl += (white*halo(d, max(px, 0.03))*1.3*(0.6 + pul*0.6) + green*halo(d, 0.11 + 0.04*pul)*0.9 + green*halo(d, 0.32)*0.12)*bright;
        // its emitter: a small glowing node on the ring
        float te;
        float de = rayPt(ro, rd, a, te);
        fxl += (pale*halo(de, 0.18)*0.8 + green*halo(de, 0.6)*0.25)*lit;
      }
      // ---- the focus: a blazing, crackling knot
      float tf;
      float dF = rayPt(ro, rd, F, tf);
      float beat = 0.5 + 0.5*sin(t*9.0);
      fxl += white*halo(dF, 0.4 + 0.2*beat + 0.3*sw)*2.0*lit + green*halo(dF, 1.3)*0.6*lit;
      for (int j = 0; j < 3; j++){
        float fj = float(j);
        float k = floor(t*12.0 + fj*0.31);
        vec3 e = F + around(0.9 + 1.0*h11(k*1.7 + fj), h11(k*3.1 + fj*5.0), 1.6*(h11(k*2.3 + fj) - 0.4));
        float db2 = bolt3(ro, rd, F, e, k + fj*11.0, 0.35, px);
        fxl += boltLight(db2, green, px)*0.7*lit*step(0.25, h11(k*4.7 + fj));
      }
    }
    // ---- a clash fires the focus: a white flash and a shockwave sphere
    if (cl > 0.01){
      float tf;
      float dF = rayPt(ro, rd, F, tf);
      float Rr = 0.6 + 5.0*(1.0 - cl);
      float sh = clamp(0.25/sqrt(max(Rr*Rr - dF*dF, 0.06)), 0.0, 1.2)*smoothstep(Rr + 0.3, Rr - 0.1, dF);
      fxl += white*halo(dF, 2.2)*pow(cl, 3.0)*2.0 + mix(white, green, 1.0 - cl)*sh*cl*1.4;
    }
    light += fxl*fx;
  }
  return light;
}
// the beam: Sol's glow and core from the file, its sheath rippling, surges racing up it from
// the focus
vec3 sig_glow(vec3 g, vec2 b, float r, float px){
  float t = uTime, L = uLength;
  float surge = 0.0;
  for (int j = 0; j < 3; j++){
    float y = SL_F + fract(t*0.9 + float(j)/3.0)*(L - SL_F + 4.0) - 2.0;
    surge += halo(abs(b.y - y), 1.0 + 0.6*clamp(uSwing, 0.0, 1.0));
  }
  float rip = 0.5 + 0.5*sin(b.y*1.6 - t*14.0);
  float above = smoothstep(SL_F - 0.6, SL_F + 0.6, b.y);
  return g*(0.85 + 0.15*rip + 0.6*surge*above + 0.5*clamp(uClash, 0.0, 1.0));
}
vec3 sig_core(vec3 c, vec2 b, float px){ return c*(1.0 + 0.08*(vn(vec2(b.y*2.0, floor(uTime*30.0))) - 0.5)); }

// ---- round 9: the big effects layer ----
// Round 11 (Creyon): its own look, Sol's way: its round-8 effects as part 0, its own parts
// (`part(k)`, the `// @part` lines) and Sol's pieces from its file; nothing shared with the others.
vec3 sig_vol(vec3 ro, vec3 rd, float px){
  vec3 col = r8_vol(ro, rd, px)*part(0.0);
  float L = uLength, t = uTime, sw = clamp(uSwing, 0.0, 1.0), cl = clamp(uClash, 0.0, 1.0);
  float y0 = L*0.2, y1 = L*0.97;
  vec3 fx = vec3(0.0);
  fx += fxSparks(ro, rd, px, L*0.3, y1, 18.0, 6.0, 1.1, 4.8, vec3(0.85, 1.0, 0.85))*part(1.0);
  // round 15: each surge racing up the beam swells into a ball of green plasma round it, and a
  // charge of light gathers at the focus before it fires
  float ta;
  for (int j = 0; j < 3; j++){
    float y = SL_F + fract(t*0.9 + float(j)/3.0)*(L - SL_F + 4.0) - 2.0;
    if (y > L) continue;
    float d = rayPtRaw(ro, rd, vec3(0.0, y, 0.0), ta);
    fx += (vec3(0.25, 1.0, 0.3)*halo(d, 1.2 + 0.6*sw)*0.55 + vec3(0.8, 1.0, 0.8)*halo(abs(d - 1.1), 0.08)*0.5)*part(0.0);
  }
  float charge = fract(t*0.9);
  float dF = rayPtRaw(ro, rd, vec3(0.0, SL_F, 0.0), ta);
  fx += vec3(0.25, 1.0, 0.3)*halo(abs(dF - 2.5*(1.0 - charge)), 0.12)*charge*0.9*part(0.0);
  return col + fx*uFx;
}
