use glam::{Mat4, Quat, Vec3};
use renderer::vertex::Vertex;

use crate::blendshape::BlendShapeGroup;
use crate::bone::HumanoidBones;
use crate::spring_bone::SpringBoneGroup;

/// glTFスキンのジョイント情報
pub struct SkinJoint {
    /// glTFノードインデックス
    pub node_index: usize,
    /// バインドポーズの逆行列
    pub inverse_bind_matrix: Mat4,
}

/// マテリアル情報
pub struct Material {
    /// ベースカラー (RGBA)
    pub base_color: [f32; 4],
    /// ベースカラーテクスチャ (RGBA画像)
    pub base_color_texture: Option<image::DynamicImage>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
        }
    }
}

/// メッシュプリミティブのデータ
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub morph_targets: Vec<MorphTargetData>,
    pub material_index: Option<usize>,
}

/// MorphTargetの差分データ
pub struct MorphTargetData {
    pub position_deltas: Vec<[f32; 3]>,
    pub normal_deltas: Vec<[f32; 3]>,
}

/// glTFノードの変換情報
pub struct NodeTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub children: Vec<usize>,
}

/// VRMモデル全体のデータ
pub struct VrmModel {
    pub meshes: Vec<MeshData>,
    pub materials: Vec<Material>,
    pub skins: Vec<SkinJoint>,
    pub humanoid_bones: HumanoidBones,
    pub blend_shapes: BlendShapeGroup,
    pub node_transforms: Vec<NodeTransform>,
    pub spring_bone_groups: Vec<SpringBoneGroup>,
}
