#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct BlitUniforms {
    scale: f32,
};

@group(0) @binding(0) var in_texture: texture_2d<f32>;
@group(0) @binding(1) var in_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: BlitUniforms;

@fragment
fn fs_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(in_texture, in_sampler, in.uv * uniforms.scale);
}
