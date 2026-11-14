struct Draw { rect: vec4f, color: vec4f, viewport: vec4f }
@group(0) @binding(0) var<uniform> draw: Draw;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let uv = array<vec2f, 6>(vec2f(0., 0.), vec2f(1., 0.), vec2f(0., 1.), vec2f(0., 1.), vec2f(1., 0.), vec2f(1., 1.))[i];
    let pixel = draw.rect.xy + uv * draw.rect.zw;
    return vec4f(pixel.x / draw.viewport.x * 2. - 1., 1. - pixel.y / draw.viewport.y * 2., 0., 1.);
}
@fragment fn fs() -> @location(0) vec4f { return draw.color; }
