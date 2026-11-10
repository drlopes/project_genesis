// E3 terrain: geomorph between a patch's own shape and its parent level's shape.
import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_position_local_to_clip};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> sun: vec4<f32>;
// x: morph factor (0 = own, 1 = parent), y: quadtree level, z: view mode
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> params: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) morph_offset: vec3<f32>,
    @location(4) morph_normal: vec3<f32>,
    @location(5) morph_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let f = params.x;
    let local = v.position + v.morph_offset * f;
    var out: VertexOutput;
    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(v.instance_index),
        vec4<f32>(local, 1.0),
    );
    // Patches are only translated, so local normals are world normals.
    out.normal = mix(v.normal, v.morph_normal, f);
    out.color = mix(v.color, v.morph_color, f);
    return out;
}

fn level_color(level: f32) -> vec3<f32> {
    let h = fract(level * 0.618034);
    let rgb = clamp(abs(fract(vec3<f32>(h, h + 0.6666667, h + 0.3333333)) * 6.0 - 3.0) - 1.0,
        vec3<f32>(0.0), vec3<f32>(1.0));
    return rgb * 0.7 + 0.15;
}

struct FragmentInput {
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let lambert = max(dot(n, sun.xyz), 0.0);
    var albedo = in.color.rgb;
    let mode = u32(params.z + 0.5);
    if mode == 1u {
        albedo = vec3<f32>(0.6);
    } else if mode == 2u {
        albedo = level_color(params.y);
    }
    return vec4<f32>(albedo * (0.12 + 0.88 * lambert), 1.0);
}
