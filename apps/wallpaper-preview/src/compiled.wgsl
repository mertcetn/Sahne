struct Draw { rect: vec4f, color: vec4f, viewport: vec4f }
@group(0) @binding(0) var<uniform> draw: Draw;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Vertex {
    let uv = array<vec2f, 6>(vec2f(0., 0.), vec2f(1., 0.), vec2f(0., 1.), vec2f(0., 1.), vec2f(1., 0.), vec2f(1., 1.))[i];
    let pixel = draw.rect.xy + uv * draw.rect.zw;
    var result: Vertex;
    result.position = vec4f(pixel.x / draw.viewport.x * 2. - 1., 1. - pixel.y / draw.viewport.y * 2., 0., 1.);
    result.uv = uv;
    return result;
}
@fragment fn fs() -> @location(0) vec4f { return draw.color; }
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;
@fragment fn fs_image(vertex: Vertex) -> @location(0) vec4f {
    return textureSample(image, image_sampler, vertex.uv);
}
