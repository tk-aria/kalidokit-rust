use glam::{Mat4, Vec3};

/// Shape of a collider used for spring bone collision detection.
#[derive(Debug, Clone)]
pub enum ColliderShape {
    Sphere { radius: f32 },
}

/// A collider that can push spring bone tails out of its volume.
#[derive(Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub offset: Vec3,
    pub node_index: usize,
    pub world_position: Vec3,
}

impl Collider {
    /// Update the world-space position of this collider from a node's world matrix.
    pub fn update_world_position(&mut self, node_world_matrix: &Mat4) {
        self.world_position = node_world_matrix.transform_point3(self.offset);
    }

    /// Resolve collision between a spring bone tail and this collider.
    ///
    /// If the tail is inside the collider volume, it is pushed outward along the
    /// surface normal. Returns the (possibly adjusted) tail position.
    pub fn resolve_collision(&self, tail: Vec3, hit_radius: f32) -> Vec3 {
        match self.shape {
            ColliderShape::Sphere { radius } => {
                let diff = tail - self.world_position;
                let dist = diff.length();
                let min_dist = radius + hit_radius;

                if dist < min_dist {
                    if dist > 1e-6 {
                        // Push tail out along the normal from center to tail.
                        let normal = diff / dist;
                        self.world_position + normal * min_dist
                    } else {
                        // Tail is at center; push in +Y as a safe fallback.
                        self.world_position + Vec3::Y * min_dist
                    }
                } else {
                    tail
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere_collider(radius: f32, world_position: Vec3) -> Collider {
        Collider {
            shape: ColliderShape::Sphere { radius },
            offset: Vec3::ZERO,
            node_index: 0,
            world_position,
        }
    }

    #[test]
    fn sphere_pushes_point_out() {
        let c = sphere_collider(1.0, Vec3::ZERO);
        // Tail inside sphere at (0.5, 0, 0), hit_radius = 0
        let result = c.resolve_collision(Vec3::new(0.5, 0.0, 0.0), 0.0);
        // Should be pushed out to radius 1.0 along +X
        assert!((result.x - 1.0).abs() < 1e-5);
        assert!(result.y.abs() < 1e-5);
        assert!(result.z.abs() < 1e-5);
    }

    #[test]
    fn point_outside_sphere_unchanged() {
        let c = sphere_collider(1.0, Vec3::ZERO);
        let tail = Vec3::new(2.0, 0.0, 0.0);
        let result = c.resolve_collision(tail, 0.0);
        assert!((result - tail).length() < 1e-5);
    }

    #[test]
    fn world_position_transforms_offset() {
        let mut c = Collider {
            shape: ColliderShape::Sphere { radius: 0.5 },
            offset: Vec3::new(1.0, 0.0, 0.0),
            node_index: 0,
            world_position: Vec3::ZERO,
        };
        // Translate by (3, 4, 5)
        let mat = Mat4::from_translation(Vec3::new(3.0, 4.0, 5.0));
        c.update_world_position(&mat);
        assert!((c.world_position - Vec3::new(4.0, 4.0, 5.0)).length() < 1e-5);
    }

    #[test]
    fn zero_radius_no_collision() {
        let c = sphere_collider(0.0, Vec3::ZERO);
        let tail = Vec3::new(0.5, 0.0, 0.0);
        let result = c.resolve_collision(tail, 0.0);
        // radius + hit_radius = 0, so 0.5 > 0 => no collision
        assert!((result - tail).length() < 1e-5);
    }

    #[test]
    fn point_at_center_pushed_out_safely() {
        let c = sphere_collider(1.0, Vec3::ZERO);
        // Tail exactly at collider center
        let result = c.resolve_collision(Vec3::ZERO, 0.0);
        // Should be pushed in +Y direction by radius
        assert!((result - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-5);
    }
}
