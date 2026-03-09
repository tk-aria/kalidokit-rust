use glam::{Mat4, Vec3};

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
}

impl Camera {
    pub fn build_view_proj(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.position, self.target, Vec3::Y);
        let proj = Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far);
        proj * view
    }

    pub fn to_uniform(&self, model: Mat4) -> CameraUniform {
        CameraUniform {
            view_proj: self.build_view_proj().to_cols_array_2d(),
            model: model.to_cols_array_2d(),
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 1.4, -0.7),
            target: Vec3::new(0.0, 1.0, 0.0),
            fov: 50.0,
            aspect: 16.0 / 9.0,
            near: 0.01,
            far: 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_view_proj_not_identity() {
        let cam = Camera::default();
        let vp = cam.build_view_proj();
        assert_ne!(vp, Mat4::IDENTITY);
    }

    #[test]
    fn aspect_change_affects_matrix() {
        let cam1 = Camera::default();
        let mut cam2 = Camera::default();
        cam2.aspect = 4.0 / 3.0;
        assert_ne!(cam1.build_view_proj(), cam2.build_view_proj());
    }

    #[test]
    fn uniform_is_pod() {
        let cam = Camera::default();
        let u = cam.to_uniform(Mat4::IDENTITY);
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 128); // 2 * 16 * f32(4) = 128
    }
}
