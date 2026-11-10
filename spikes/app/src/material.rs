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

// Compact vertex format (I-005, see patch.rs). High, arbitrary ids, as the Bevy docs recommend
// for custom attributes. Bevy's own POSITION/NORMAL/COLOR are not used: they are float formats.
pub const ATTRIBUTE_QPOSITION: MeshVertexAttribute =
    MeshVertexAttribute::new("QPosition", 1_870_331_211, VertexFormat::Unorm16x4);
pub const ATTRIBUTE_QMORPH_POSITION: MeshVertexAttribute =
    MeshVertexAttribute::new("QMorphPosition", 1_870_331_212, VertexFormat::Unorm16x4);
pub const ATTRIBUTE_QNORMALS: MeshVertexAttribute =
    MeshVertexAttribute::new("QNormals", 1_870_331_213, VertexFormat::Snorm16x4);
pub const ATTRIBUTE_QCOLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("QColor", 1_870_331_214, VertexFormat::Unorm8x4);
pub const ATTRIBUTE_QMORPH_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("QMorphColor", 1_870_331_215, VertexFormat::Unorm8x4);

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
            ATTRIBUTE_QPOSITION.at_shader_location(0),
            ATTRIBUTE_QMORPH_POSITION.at_shader_location(1),
            ATTRIBUTE_QNORMALS.at_shader_location(2),
            ATTRIBUTE_QCOLOR.at_shader_location(3),
            ATTRIBUTE_QMORPH_COLOR.at_shader_location(4),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        // Skirt triangles are vertical, so the winding fix-up (outward-facing test) cannot orient
        // them reliably; draw both sides. Spike shortcut: costs some overdraw.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
