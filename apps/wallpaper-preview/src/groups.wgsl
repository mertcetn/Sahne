struct Draw { rect:vec4f, color:vec4f, clip:vec4f, viewport:vec4f, params:vec4f }
@group(0) @binding(0) var<uniform> draw:Draw;
@group(1) @binding(0) var image:texture_2d<f32>;
@group(1) @binding(1) var image_sampler:sampler;
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn vs(@builtin(vertex_index) i:u32)->Vertex {
    let uv=array<vec2f,6>(vec2f(0.,0.),vec2f(1.,0.),vec2f(0.,1.),vec2f(0.,1.),vec2f(1.,0.),vec2f(1.,1.))[i];
    let pixel=draw.rect.xy+uv*draw.rect.zw;
    var result:Vertex;
    result.position=vec4f(pixel.x/draw.viewport.x*2.-1.,1.-pixel.y/draw.viewport.y*2.,0.,1.);
    result.uv=uv*draw.params.yz;
    return result;
}
fn clipped(p:vec2f)->bool { return any(p<draw.clip.xy)||any(p>=draw.clip.zw); }
@fragment fn fs_rect(v:Vertex)->@location(0) vec4f {
    if clipped(v.position.xy) {discard;}
    return draw.color;
}
@fragment fn fs_straight(v:Vertex)->@location(0) vec4f {
    if clipped(v.position.xy) {discard;}
    return textureSampleLevel(image,image_sampler,v.uv,0.);
}
@fragment fn fs_premultiplied(v:Vertex)->@location(0) vec4f {
    if clipped(v.position.xy) {discard;}
    // Stored RGB is already premultiplied in linear light. Scale RGB and alpha once.
    return textureSampleLevel(image,image_sampler,v.uv,0.)*draw.params.x;
}
