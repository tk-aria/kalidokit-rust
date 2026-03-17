use std::collections::HashMap;

use glam::Quat;

use crate::animation::AnimationClip;
use crate::bone::HumanoidBoneName;

/// Bone group categories for per-group blend weight control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoneGroup {
    /// Head, Neck, Eyes, Jaw
    Head,
    /// Spine, Chest, UpperChest
    Torso,
    /// Shoulders, upper/lower arms, hands
    Arms,
    /// Fingers (all 30 bones)
    Fingers,
    /// Upper/lower legs, feet, toes, hips
    LowerBody,
}

impl BoneGroup {
    /// Classify a bone into its group.
    pub fn of(bone: HumanoidBoneName) -> Self {
        use HumanoidBoneName::*;
        match bone {
            Head | Neck | LeftEye | RightEye | Jaw => BoneGroup::Head,
            Spine | Chest | UpperChest => BoneGroup::Torso,
            LeftShoulder | RightShoulder | LeftUpperArm | RightUpperArm | LeftLowerArm
            | RightLowerArm | LeftHand | RightHand => BoneGroup::Arms,
            LeftThumbProximal
            | LeftThumbIntermediate
            | LeftThumbDistal
            | LeftIndexProximal
            | LeftIndexIntermediate
            | LeftIndexDistal
            | LeftMiddleProximal
            | LeftMiddleIntermediate
            | LeftMiddleDistal
            | LeftRingProximal
            | LeftRingIntermediate
            | LeftRingDistal
            | LeftLittleProximal
            | LeftLittleIntermediate
            | LeftLittleDistal
            | RightThumbProximal
            | RightThumbIntermediate
            | RightThumbDistal
            | RightIndexProximal
            | RightIndexIntermediate
            | RightIndexDistal
            | RightMiddleProximal
            | RightMiddleIntermediate
            | RightMiddleDistal
            | RightRingProximal
            | RightRingIntermediate
            | RightRingDistal
            | RightLittleProximal
            | RightLittleIntermediate
            | RightLittleDistal => BoneGroup::Fingers,
            Hips | LeftUpperLeg | RightUpperLeg | LeftLowerLeg | RightLowerLeg | LeftFoot
            | RightFoot | LeftToes | RightToes => BoneGroup::LowerBody,
        }
    }
}

/// Blend weight configuration: how much tracking vs. idle animation per bone group.
///
/// `tracking_weight` = 1.0 means fully tracking, 0.0 means fully idle animation.
pub struct BlendConfig {
    weights: HashMap<BoneGroup, f32>,
}

impl Default for BlendConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();
        // Head/face: tracking-dominant (webcam tracks face well)
        weights.insert(BoneGroup::Head, 0.8);
        // Torso: blend both (tracking captures rough torso motion)
        weights.insert(BoneGroup::Torso, 0.6);
        // Arms: tracking-dominant when detected
        weights.insert(BoneGroup::Arms, 0.7);
        // Fingers: tracking-dominant when detected
        weights.insert(BoneGroup::Fingers, 0.9);
        // Lower body: idle-dominant (webcam rarely captures legs)
        weights.insert(BoneGroup::LowerBody, 0.1);
        Self { weights }
    }
}

impl BlendConfig {
    /// Get tracking weight for a bone. Returns 1.0 (fully tracking) if not configured.
    pub fn tracking_weight(&self, bone: HumanoidBoneName) -> f32 {
        let group = BoneGroup::of(bone);
        self.weights.get(&group).copied().unwrap_or(1.0)
    }
}

/// Animation playback and blending engine.
pub struct AnimationPlayer {
    clip: AnimationClip,
    /// Current playback time in seconds.
    time: f32,
    /// Playback speed multiplier.
    pub speed: f32,
    /// Whether the animation loops.
    pub looping: bool,
    /// Per-bone-group blend weights (tracking vs. idle).
    pub blend_config: BlendConfig,
    /// Whether idle animation is active.
    pub enabled: bool,
    /// VRM bind pose rotations per bone (for applying delta rotations).
    vrm_bind_pose: HashMap<HumanoidBoneName, Quat>,
}

impl AnimationPlayer {
    pub fn new(clip: AnimationClip) -> Self {
        Self {
            clip,
            time: 0.0,
            speed: 1.0,
            looping: true,
            blend_config: BlendConfig::default(),
            enabled: true,
            vrm_bind_pose: HashMap::new(),
        }
    }

    /// Set the VRM bind pose rotations so delta rotations can be applied correctly.
    pub fn set_vrm_bind_pose(&mut self, bind_pose: HashMap<HumanoidBoneName, Quat>) {
        self.vrm_bind_pose = bind_pose;
    }

    /// Advance the animation by `delta_seconds`.
    pub fn update(&mut self, delta_seconds: f32) {
        if !self.enabled {
            return;
        }
        self.time += delta_seconds * self.speed;
        if self.looping && self.clip.duration > 0.0 {
            self.time %= self.clip.duration;
        } else {
            self.time = self.time.min(self.clip.duration);
        }
    }

    /// Sample the current pose: returns final rotation for each bone at the current time.
    ///
    /// The stored keyframes are delta rotations. This method applies:
    /// `final = vrm_bind_rotation * delta`
    pub fn sample(&self) -> HashMap<HumanoidBoneName, Quat> {
        let mut pose = HashMap::new();
        if !self.enabled {
            return pose;
        }

        for channel in &self.clip.channels {
            if let Some(delta) = sample_rotation(&channel.times, &channel.rotations, self.time) {
                let bind = self
                    .vrm_bind_pose
                    .get(&channel.bone)
                    .copied()
                    .unwrap_or(Quat::IDENTITY);
                pose.insert(channel.bone, (bind * delta).normalize());
            }
        }

        pose
    }

    /// Get the blend weight for tracking (vs idle) for a given bone.
    ///
    /// When tracking data is available for a bone, use this weight to blend:
    /// `final = idle_quat.slerp(tracking_quat, tracking_weight)`
    ///
    /// When tracking data is NOT available, idle animation is used directly.
    pub fn tracking_weight(&self, bone: HumanoidBoneName) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        self.blend_config.tracking_weight(bone)
    }

    pub fn clip_name(&self) -> &str {
        &self.clip.name
    }

    pub fn duration(&self) -> f32 {
        self.clip.duration
    }
}

/// Sample a rotation channel at a given time using linear interpolation (slerp).
fn sample_rotation(times: &[f32], rotations: &[Quat], t: f32) -> Option<Quat> {
    if times.is_empty() || rotations.is_empty() {
        return None;
    }

    // Before first keyframe
    if t <= times[0] {
        return Some(rotations[0]);
    }

    // After last keyframe
    if t >= *times.last().unwrap() {
        return Some(*rotations.last().unwrap());
    }

    // Find surrounding keyframes via binary search
    let idx = match times.binary_search_by(|k| k.partial_cmp(&t).unwrap()) {
        Ok(i) => return Some(rotations[i]),
        Err(i) => i,
    };

    // idx is the insertion point, so keyframes[idx-1] < t < keyframes[idx]
    let i0 = idx - 1;
    let i1 = idx;
    let t0 = times[i0];
    let t1 = times[i1];
    let factor = (t - t0) / (t1 - t0);

    Some(rotations[i0].slerp(rotations[i1], factor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_group_classification() {
        assert_eq!(BoneGroup::of(HumanoidBoneName::Head), BoneGroup::Head);
        assert_eq!(BoneGroup::of(HumanoidBoneName::Spine), BoneGroup::Torso);
        assert_eq!(
            BoneGroup::of(HumanoidBoneName::LeftUpperArm),
            BoneGroup::Arms
        );
        assert_eq!(
            BoneGroup::of(HumanoidBoneName::LeftThumbProximal),
            BoneGroup::Fingers
        );
        assert_eq!(BoneGroup::of(HumanoidBoneName::Hips), BoneGroup::LowerBody);
        assert_eq!(
            BoneGroup::of(HumanoidBoneName::LeftFoot),
            BoneGroup::LowerBody
        );
    }

    #[test]
    fn sample_rotation_interpolates() {
        let times = vec![0.0, 1.0];
        let q0 = Quat::IDENTITY;
        let q1 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let rotations = vec![q0, q1];

        // At t=0.5, should be halfway between identity and 90-degree Y rotation
        let result = sample_rotation(&times, &rotations, 0.5).unwrap();
        let expected = q0.slerp(q1, 0.5);
        assert!(result.dot(expected).abs() > 0.999);
    }

    #[test]
    fn sample_rotation_clamps_to_bounds() {
        let times = vec![1.0, 2.0];
        let q0 = Quat::IDENTITY;
        let q1 = Quat::from_rotation_y(1.0);
        let rotations = vec![q0, q1];

        // Before first keyframe: returns first
        let result = sample_rotation(&times, &rotations, 0.0).unwrap();
        assert!(result.dot(q0).abs() > 0.999);

        // After last keyframe: returns last
        let result = sample_rotation(&times, &rotations, 5.0).unwrap();
        assert!(result.dot(q1).abs() > 0.999);
    }

    #[test]
    fn default_blend_config_values() {
        let cfg = BlendConfig::default();
        // Lower body should be idle-dominant
        assert!(cfg.tracking_weight(HumanoidBoneName::LeftUpperLeg) < 0.5);
        // Head should be tracking-dominant
        assert!(cfg.tracking_weight(HumanoidBoneName::Head) > 0.5);
    }
}
