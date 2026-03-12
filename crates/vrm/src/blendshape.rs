use std::collections::HashMap;

use crate::error::VrmError;

/// VRM BlendShapeプリセット名
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendShapePreset {
    Blink,
    BlinkL,
    BlinkR,
    A,
    I,
    U,
    E,
    O,
    Joy,
    Angry,
    Sorrow,
    Fun,
    Neutral,
}

impl BlendShapePreset {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "blink" => Some(Self::Blink),
            "blink_l" => Some(Self::BlinkL),
            "blink_r" => Some(Self::BlinkR),
            "a" => Some(Self::A),
            "i" => Some(Self::I),
            "u" => Some(Self::U),
            "e" => Some(Self::E),
            "o" => Some(Self::O),
            "joy" => Some(Self::Joy),
            "angry" => Some(Self::Angry),
            "sorrow" => Some(Self::Sorrow),
            "fun" => Some(Self::Fun),
            "neutral" => Some(Self::Neutral),
            _ => None,
        }
    }
}

/// BlendShapeバインディング: どのメッシュのどのMorphTargetにどの重みで反映するか
#[derive(Debug, Clone)]
pub struct BlendShapeBinding {
    pub mesh_index: usize,
    pub morph_target_index: usize,
    pub weight: f32,
}

/// BlendShapeグループ: プリセット→バインディング群のマッピング
pub struct BlendShapeGroup {
    groups: HashMap<BlendShapePreset, Vec<BlendShapeBinding>>,
    current_weights: HashMap<BlendShapePreset, f32>,
}

impl BlendShapeGroup {
    /// VRM拡張JSONからBlendShapeグループを構築
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> Result<Self, VrmError> {
        let groups_array = vrm_ext
            .get("blendShapeMaster")
            .and_then(|bsm| bsm.get("blendShapeGroups"))
            .and_then(|g| g.as_array())
            .ok_or_else(|| {
                VrmError::MissingExtension("blendShapeMaster.blendShapeGroups".into())
            })?;

        let mut groups = HashMap::new();
        for group in groups_array {
            let preset_name = group
                .get("presetName")
                .and_then(|p| p.as_str())
                .unwrap_or("");

            if let Some(preset) = BlendShapePreset::parse(preset_name) {
                let binds = group
                    .get("binds")
                    .and_then(|b| b.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|bind| {
                                let mesh = bind.get("mesh")?.as_u64()? as usize;
                                let index = bind.get("index")?.as_u64()? as usize;
                                let weight = bind.get("weight")?.as_f64()? as f32 / 100.0;
                                Some(BlendShapeBinding {
                                    mesh_index: mesh,
                                    morph_target_index: index,
                                    weight,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                groups.insert(preset, binds);
            }
        }

        Ok(Self {
            groups,
            current_weights: HashMap::new(),
        })
    }

    /// プリセットの現在値を取得 (デフォルト0.0)
    pub fn get(&self, preset: BlendShapePreset) -> f32 {
        self.current_weights.get(&preset).copied().unwrap_or(0.0)
    }

    /// プリセットの重みを設定 (0.0〜1.0)
    pub fn set(&mut self, preset: BlendShapePreset, value: f32) {
        self.current_weights.insert(preset, value.clamp(0.0, 1.0));
    }

    /// 全MorphTargetの重み配列を取得 (GPU転送用)
    pub fn get_all_weights(&self, num_targets: usize) -> Vec<f32> {
        let mut weights = vec![0.0f32; num_targets];
        for (preset, &value) in &self.current_weights {
            if let Some(bindings) = self.groups.get(preset) {
                for binding in bindings {
                    if binding.morph_target_index < num_targets {
                        weights[binding.morph_target_index] += value * binding.weight;
                    }
                }
            }
        }
        // Clamp to 0..1
        for w in &mut weights {
            *w = w.clamp(0.0, 1.0);
        }
        weights
    }

    /// デバッグ用: 全プリセットのバインディング情報を返す
    #[allow(clippy::type_complexity)]
    pub fn debug_bindings(&self) -> Vec<(String, Vec<(usize, usize, f32)>)> {
        self.groups
            .iter()
            .map(|(preset, bindings)| {
                let name = format!("{:?}", preset);
                let binds: Vec<(usize, usize, f32)> = bindings
                    .iter()
                    .map(|b| (b.mesh_index, b.morph_target_index, b.weight))
                    .collect();
                (name, binds)
            })
            .collect()
    }

    /// 特定メッシュのMorphTarget重み配列を取得 (per-mesh GPU転送用)
    pub fn get_weights_for_mesh(&self, mesh_index: usize, num_targets: usize) -> Vec<f32> {
        let mut weights = vec![0.0f32; num_targets];
        for (preset, &value) in &self.current_weights {
            if let Some(bindings) = self.groups.get(preset) {
                for binding in bindings {
                    if binding.mesh_index == mesh_index
                        && binding.morph_target_index < num_targets
                    {
                        weights[binding.morph_target_index] += value * binding.weight;
                    }
                }
            }
        }
        for w in &mut weights {
            *w = w.clamp(0.0, 1.0);
        }
        weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_from_str() {
        assert_eq!(
            BlendShapePreset::parse("blink"),
            Some(BlendShapePreset::Blink)
        );
        assert_eq!(BlendShapePreset::parse("a"), Some(BlendShapePreset::A));
        assert_eq!(BlendShapePreset::parse("unknown"), None);
    }

    #[test]
    fn set_and_get_weights() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "blendShapeMaster": {
                    "blendShapeGroups": [
                        {
                            "presetName": "blink",
                            "binds": [
                                { "mesh": 0, "index": 1, "weight": 100 }
                            ]
                        }
                    ]
                }
            }"#,
        )
        .unwrap();

        let mut group = BlendShapeGroup::from_vrm_json(&json).unwrap();
        group.set(BlendShapePreset::Blink, 0.5);
        let weights = group.get_all_weights(4);
        assert_eq!(weights.len(), 4);
        assert!((weights[1] - 0.5).abs() < 1e-6);
        assert!((weights[0]).abs() < 1e-6);
    }

    #[test]
    fn multiple_presets_add_weights() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "blendShapeMaster": {
                    "blendShapeGroups": [
                        {
                            "presetName": "blink",
                            "binds": [{ "mesh": 0, "index": 0, "weight": 100 }]
                        },
                        {
                            "presetName": "joy",
                            "binds": [{ "mesh": 0, "index": 0, "weight": 50 }]
                        }
                    ]
                }
            }"#,
        )
        .unwrap();

        let mut group = BlendShapeGroup::from_vrm_json(&json).unwrap();
        group.set(BlendShapePreset::Blink, 0.5);
        group.set(BlendShapePreset::Joy, 0.4);
        let weights = group.get_all_weights(2);
        // blink: 0.5 * 1.0 = 0.5, joy: 0.4 * 0.5 = 0.2, total = 0.7
        assert!((weights[0] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn missing_blend_shape_master_returns_error() {
        let json: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        assert!(BlendShapeGroup::from_vrm_json(&json).is_err());
    }
}
