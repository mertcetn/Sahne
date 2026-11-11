@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Vertex {
    let uv = array<vec2f, 3>(vec2f(0., 0.), vec2f(2., 0.), vec2f(0., 2.))[i];
    return Vertex(vec4f(uv.x * 2. - 1., 1. - uv.y * 2., 0., 1.), uv);
}
@fragment fn fs(v: Vertex) -> @location(0) vec4f {
    let uv = v.uv;
    var color = vec4f(0.055, 0.085, 0.10, 1.);
    if (uv.x > 0.06 && uv.x < 0.44 && uv.y > 0.42 && uv.y < 0.85) { color = vec4f(0.1, 0.45, 0.38, 1.); }
    if (uv.x > 0.56 && uv.x < 0.90 && uv.y > 0.42 && uv.y < 0.85) { color = textureSample(image, image_sampler, (uv - vec2f(0.56, 0.42)) / vec2f(0.34, 0.43)); }
    return color;
}
