//! Terrain material: geomorphing vertex shader plus simple sun lighting and debug views.
//!
//! Each patch vertex carries its own state and its parent level's state (position offset, normal,
//! colour). `params.x` is the morph factor: 0 = this level's shape, 1 = the parent's. Patches share
//! one material per (level, morph step), so settled patches still batch.

use bevy::{
    asset::embedded_asset,
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, VertexFormat,
    },
    shader::ShaderRef,
};

// High, arbitrary ids, as the Bevy docs recommend for custom attributes.
pub const ATTRIBUTE_MORPH_OFFSET: MeshVertexAttribute =
    MeshVertexAttribute::new("MorphOffset", 1_870_331_201, VertexFormat::Float32x3);
pub const ATTRIBUTE_MORPH_NORMAL: MeshVertexAttribute =
    MeshVertexAttribute::new("MorphNormal", 1_870_331_202, VertexFormat::Float32x3);
pub const ATTRIBUTE_MORPH_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("MorphColor", 1_870_331_203, VertexFormat::Float32x4);

/// Embedded so the scripted runs, which launch the exe directly, need no assets folder.
const SHADER: &str = "embedded://spike_app/terrain.wesl";

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain.wesl");
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainMaterial {
    /// xyz: unit direction towards the sun, world space.
    #[uniform(0)]
    pub sun: Vec4,
    /// x: morph factor, y: quadtree level, z: view mode (0 shaded, 1 grey, 2 colour by level).
    #[uniform(1)]
    pub params: Vec4,
}

impl Material for TerrainMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
            ATTRIBUTE_MORPH_OFFSET.at_shader_location(3),
            ATTRIBUTE_MORPH_NORMAL.at_shader_location(4),
            ATTRIBUTE_MORPH_COLOR.at_shader_location(5),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        // Skirt triangles are vertical, so the winding fix-up (outward-facing test) cannot orient
        // them reliably; draw both sides. Spike shortcut: costs some overdraw.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
