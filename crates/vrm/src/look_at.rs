use glam::Quat;

use crate::error::VrmError;

/// 視線カーブパラメータ
#[derive(Debug, Clone, Copy)]
pub struct CurveRange {
    /// 入力の最大角度(度)
    pub input_max_value: f32,
    /// 出力の最大角度(度)
    pub output_scale: f32,
}

impl Default for CurveRange {
    fn default() -> Self {
        Self {
            input_max_value: 90.0,
            output_scale: 10.0,
        }
    }
}

/// 視線タイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookAtType {
    Bone,
    BlendShape,
}

/// 視線のオイラー角 (度)
pub struct EulerAngles {
    pub yaw: f32,
    pub pitch: f32,
}

/// 視線制御
pub struct LookAtApplyer {
    pub look_at_type: LookAtType,
    pub horizontal_inner: CurveRange,
    pub horizontal_outer: CurveRange,
    pub vertical_up: CurveRange,
    pub vertical_down: CurveRange,
}

impl LookAtApplyer {
    /// VRM拡張JSONからパース
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> Result<Self, VrmError> {
        let first_person = vrm_ext
            .get("firstPerson")
            .ok_or_else(|| VrmError::MissingExtension("firstPerson".into()))?;

        let type_name = first_person
            .get("lookAtTypeName")
            .and_then(|v| v.as_str())
            .unwrap_or("Bone");

        let look_at_type = match type_name {
            "BlendShape" => LookAtType::BlendShape,
            _ => LookAtType::Bone,
        };

        fn parse_curve(json: &serde_json::Value, key: &str) -> CurveRange {
            json.get(key)
                .map(|v| CurveRange {
                    input_max_value: v.get("xRange").and_then(|x| x.as_f64()).unwrap_or(90.0)
                        as f32,
                    output_scale: v.get("yRange").and_then(|y| y.as_f64()).unwrap_or(10.0) as f32,
                })
                .unwrap_or_default()
        }

        Ok(Self {
            look_at_type,
            horizontal_inner: parse_curve(first_person, "lookAtHorizontalInner"),
            horizontal_outer: parse_curve(first_person, "lookAtHorizontalOuter"),
            vertical_up: parse_curve(first_person, "lookAtVerticalUp"),
            vertical_down: parse_curve(first_person, "lookAtVerticalDown"),
        })
    }

    /// 視線のオイラー角からボーン回転Quaternionを計算
    pub fn apply(&self, euler: &EulerAngles) -> Quat {
        let yaw_deg = euler.yaw.clamp(-90.0, 90.0);
        let pitch_deg = euler.pitch.clamp(-90.0, 90.0);

        // Apply horizontal curve
        let yaw_range = if yaw_deg >= 0.0 {
            &self.horizontal_outer
        } else {
            &self.horizontal_inner
        };
        let yaw_output = if yaw_range.input_max_value > 0.0 {
            (yaw_deg.abs() / yaw_range.input_max_value).min(1.0) * yaw_range.output_scale
        } else {
            0.0
        };
        let yaw_rad = yaw_output.copysign(yaw_deg).to_radians();

        // Apply vertical curve
        let pitch_range = if pitch_deg >= 0.0 {
            &self.vertical_up
        } else {
            &self.vertical_down
        };
        let pitch_output = if pitch_range.input_max_value > 0.0 {
            (pitch_deg.abs() / pitch_range.input_max_value).min(1.0) * pitch_range.output_scale
        } else {
            0.0
        };
        let pitch_rad = pitch_output.copysign(pitch_deg).to_radians();

        Quat::from_euler(glam::EulerRot::YXZ, yaw_rad, pitch_rad, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_zero_returns_identity() {
        let applyer = LookAtApplyer {
            look_at_type: LookAtType::Bone,
            horizontal_inner: CurveRange::default(),
            horizontal_outer: CurveRange::default(),
            vertical_up: CurveRange::default(),
            vertical_down: CurveRange::default(),
        };
        let q = applyer.apply(&EulerAngles {
            yaw: 0.0,
            pitch: 0.0,
        });
        let diff = q.dot(Quat::IDENTITY).abs();
        assert!((diff - 1.0).abs() < 1e-5);
    }

    #[test]
    fn apply_extreme_values_no_nan() {
        let applyer = LookAtApplyer {
            look_at_type: LookAtType::Bone,
            horizontal_inner: CurveRange::default(),
            horizontal_outer: CurveRange::default(),
            vertical_up: CurveRange::default(),
            vertical_down: CurveRange::default(),
        };
        let q = applyer.apply(&EulerAngles {
            yaw: 180.0,
            pitch: -180.0,
        });
        assert!(!q.x.is_nan());
        assert!(!q.y.is_nan());
        assert!(!q.z.is_nan());
        assert!(!q.w.is_nan());
    }

    #[test]
    fn from_vrm_json_parses() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "firstPerson": {
                    "lookAtTypeName": "Bone",
                    "lookAtHorizontalInner": { "xRange": 90, "yRange": 10 },
                    "lookAtHorizontalOuter": { "xRange": 90, "yRange": 10 },
                    "lookAtVerticalUp": { "xRange": 90, "yRange": 10 },
                    "lookAtVerticalDown": { "xRange": 90, "yRange": 10 }
                }
            }"#,
        )
        .unwrap();

        let applyer = LookAtApplyer::from_vrm_json(&json).unwrap();
        assert_eq!(applyer.look_at_type, LookAtType::Bone);
    }
}
