use glam::{Mat4, Quat, Vec3};
use renderer::vertex::Vertex;

/// glTFスキンのジョイント情報
pub struct SkinJoint {
    /// glTFノードインデックス
    pub node_index: usize,
    /// バインドポーズの逆行列
    pub inverse_bind_matrix: Mat4,
}

/// メッシュプリミティブのデータ
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub morph_targets: Vec<MorphTargetData>,
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
    pub skins: Vec<SkinJoint>,
    pub node_transforms: Vec<NodeTransform>,
}
