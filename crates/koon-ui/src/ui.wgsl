struct View {
  size: vec4f,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var glyphs: texture_2d<f32>;
@group(0) @binding(2) var images: texture_2d<f32>;
@group(0) @binding(3) var smp: sampler;

struct In {
  @location(0) rect: vec4f,
  @location(1) radii: vec4f,
  @location(2) color: vec4f,
  @location(3) p: vec4f,
  @location(4) uv: vec4f,
  @location(5) clip: vec4f,
}

struct Out {
  @builtin(position) pos: vec4f,
  @location(0) local: vec2f,
  @location(1) @interpolate(flat) rect: vec4f,
  @location(2) @interpolate(flat) radii: vec4f,
  @location(3) @interpolate(flat) color: vec4f,
  @location(4) @interpolate(flat) p: vec4f,
  @location(5) uv: vec2f,
  @location(6) @interpolate(flat) clip: vec4f,
  @location(7) screen: vec2f,
}

fn corner(i: u32) -> vec2f {
  switch i {
    case 0u: { return vec2f(0.0, 0.0); }
    case 1u, 4u: { return vec2f(1.0, 0.0); }
    case 2u, 3u: { return vec2f(0.0, 1.0); }
    default: { return vec2f(1.0, 1.0); }
  }
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, a: In) -> Out {
  var o: Out;
  let q = corner(vi);
  let kind = a.p.x;
  var pad = 1.0;
  if (kind > 1.5 && kind < 2.5) { pad = a.p.y * 1.5 + abs(a.p.z) + 2.0; }
  var pos: vec2f;
  if (kind > 5.5) {
    let e = vec2f(a.rect.z + a.rect.w * 0.5 + 2.0);
    pos = mix(a.rect.xy - e, a.rect.xy + e, q);
  } else if (kind > 4.5) {
    let pa = a.rect.xy;
    let pb = a.rect.zw;
    let lo = min(pa, pb) - vec2f(a.p.y + 2.0);
    let hi = max(pa, pb) + vec2f(a.p.y + 2.0);
    pos = mix(lo, hi, q);
  } else if (kind > 2.5) {
    let c = a.rect.xy + a.rect.zw * 0.5;
    let rel = (q - 0.5) * a.rect.zw;
    let s = sin(a.p.w);
    let co = cos(a.p.w);
    pos = c + vec2f(rel.x * co - rel.y * s, rel.x * s + rel.y * co);
    o.uv = mix(a.uv.xy, a.uv.zw, q);
  } else {
    let lo = a.rect.xy - vec2f(pad);
    let hi = a.rect.xy + a.rect.zw + vec2f(pad);
    pos = mix(lo, hi, q);
  }
  if (kind > 1.5 && kind < 2.5) {
    let lo = a.rect.xy + vec2f(0.0, a.p.w) - vec2f(pad);
    let hi = a.rect.xy + a.rect.zw + vec2f(0.0, a.p.w) + vec2f(pad);
    pos = mix(lo, hi, q);
  }
  o.local = pos;
  o.screen = pos;
  o.rect = a.rect;
  o.radii = a.radii;
  o.color = a.color;
  o.p = a.p;
  o.clip = a.clip;
  o.pos = vec4f(pos.x / view.size.x * 2.0 - 1.0, 1.0 - pos.y / view.size.y * 2.0, 0.0, 1.0);
  return o;
}

fn sd_round(p: vec2f, b: vec2f, r4: vec4f) -> f32 {
  var r: f32;
  if (p.x > 0.0) { r = select(r4.y, r4.z, p.y > 0.0); } else { r = select(r4.x, r4.w, p.y > 0.0); }
  r = min(r, min(b.x, b.y));
  let q = abs(p) - b + r;
  return min(max(q.x, q.y), 0.0) + length(max(q, vec2f(0.0))) - r;
}

fn gaussian(x: f32, sigma: f32) -> f32 {
  let pi = 3.141592653589793;
  return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * pi) * sigma);
}

fn erf2(x: vec2f) -> vec2f {
  let s = sign(x);
  let a = abs(x);
  var y = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
  y = y * y;
  return s - s / (y * y);
}

fn shadow_x(x: f32, y: f32, sigma: f32, c: f32, half_size: vec2f) -> f32 {
  let delta = min(half_size.y - c - abs(y), 0.0);
  let curved = half_size.x - c + sqrt(max(0.0, c * c - delta * delta));
  let integral = 0.5 + 0.5 * erf2((x + vec2f(-curved, curved)) * (sqrt(0.5) / sigma));
  return integral.y - integral.x;
}

fn shadow(lower: vec2f, upper: vec2f, point0: vec2f, sigma: f32, c: f32) -> f32 {
  let center = (lower + upper) * 0.5;
  let half_size = (upper - lower) * 0.5;
  let point = point0 - center;
  let low = point.y - half_size.y;
  let high = point.y + half_size.y;
  let start = clamp(-3.0 * sigma, low, high);
  let end = clamp(3.0 * sigma, low, high);
  let step = (end - start) / 4.0;
  var y = start + step * 0.5;
  var value = 0.0;
  for (var i = 0; i < 4; i++) {
    value += shadow_x(point.x, point.y - y, sigma, c, half_size) * gaussian(y, sigma) * step;
    y += step;
  }
  return value;
}

fn seg_dist(p: vec2f, a: vec2f, b: vec2f) -> f32 {
  let pa = p - a;
  let ba = b - a;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
  return length(pa - ba * h);
}

fn arc_dist(q: vec2f, r: f32, half_width: f32, start: f32, sweep: f32) -> f32 {
  let tau = 6.283185307179586;
  let band = abs(length(q) - r) - half_width;
  if (sweep >= tau) { return band; }
  let rel = atan2(q.y, q.x) - start;
  let t = rel - floor(rel / tau) * tau;
  if (t <= sweep) { return band; }
  let e0 = r * vec2f(cos(start), sin(start));
  let e1 = r * vec2f(cos(start + sweep), sin(start + sweep));
  return min(length(q - e0), length(q - e1)) - half_width;
}

@fragment
fn fs(i: Out) -> @location(0) vec4f {
  let g = textureSampleLevel(glyphs, smp, i.uv, 0.0).r;
  let im = textureSampleLevel(images, smp, i.uv, 0.0);
  if (i.screen.x < i.clip.x || i.screen.y < i.clip.y || i.screen.x >= i.clip.z || i.screen.y >= i.clip.w) { discard; }
  let half_view = view.size.xy * 0.5;
  let m = clamp(0.5 - sd_round(i.screen - half_view, half_view, vec4f(view.size.z)), 0.0, 1.0);
  let kind = i.p.x;
  let c = i.color;
  var a = 0.0;
  if (kind < 0.5) {
    let half_size = i.rect.zw * 0.5;
    let d = sd_round(i.local - i.rect.xy - half_size, half_size, i.radii);
    a = clamp(0.5 - d, 0.0, 1.0);
  } else if (kind < 1.5) {
    let half_size = i.rect.zw * 0.5;
    let p = i.local - i.rect.xy - half_size;
    let d = sd_round(p, half_size, i.radii);
    let w = i.p.y;
    let inner = sd_round(p, max(half_size - vec2f(w), vec2f(0.0)), max(i.radii - vec4f(w), vec4f(0.0)));
    a = clamp(0.5 - d, 0.0, 1.0) * (1.0 - clamp(0.5 - inner, 0.0, 1.0));
  } else if (kind < 2.5) {
    let sigma = max(i.p.y * 0.5, 0.01);
    let spread = i.p.z;
    let lower = i.rect.xy - vec2f(spread) + vec2f(0.0, i.p.w);
    let upper = i.rect.xy + i.rect.zw + vec2f(spread) + vec2f(0.0, i.p.w);
    let r = max(i.radii.x + spread, 0.0);
    a = shadow(lower, upper, i.local, sigma, r);
    let half_box = i.rect.zw * 0.5;
    let inside = sd_round(i.local - i.rect.xy - half_box, half_box, i.radii);
    a *= clamp(0.5 + inside, 0.0, 1.0);
  } else if (kind < 3.5) {
    a = g;
  } else if (kind < 4.5) {
    let pm = im * c.a * m;
    if (pm.a < 0.002) { discard; }
    return pm;
  } else if (kind < 5.5) {
    let d = seg_dist(i.local, i.rect.xy, i.rect.zw);
    a = clamp(i.p.y * 0.5 - d + 0.5, 0.0, 1.0);
    if (i.p.z > 0.5) {
      let ba = i.rect.zw - i.rect.xy;
      let t = dot(i.local - i.rect.xy, ba) / max(dot(ba, ba), 1e-6);
      if (t < 0.0 || t >= 1.0) { discard; }
    }
  } else {
    a = clamp(0.5 - arc_dist(i.local - i.rect.xy, i.rect.z, i.rect.w * 0.5, i.p.y, i.p.z), 0.0, 1.0);
  }
  let alpha = a * c.a * m;
  if (alpha < 0.002) { discard; }
  return vec4f(c.rgb * alpha, alpha);
}
