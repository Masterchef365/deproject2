use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

/// Camera controller and parameters
#[derive(Default, Copy, Clone)]
pub struct Camera {
    pub proj: Perspective,
    pub view: ArcBall,
    pub control: ArcBallController,
}

impl Camera {
    /// Return the projection matrix of this camera
    pub fn projection(&self, width: f32, height: f32) -> Mat4 {
        self.proj.matrix(width, height)
    }

    /// Return the view matrix of this camera
    pub fn view(&self) -> Mat4 {
        self.view.matrix()
    }

    /// Pivot the camera by the given mouse pointer delta
    pub fn pivot(&mut self, delta_x: f32, delta_y: f32) {
        self.control.pivot(&mut self.view, delta_x, delta_y)
    }

    /// Pan the camera by the given mouse pointer delta
    pub fn pan(&mut self, delta_x: f32, delta_y: f32, rate_z: f32) {
        self.control.pan(&mut self.view, delta_x, delta_y, rate_z)
    }

    /// Zoom the camera by the given mouse scroll delta
    pub fn zoom(&mut self, delta: f32) {
        self.control.zoom(&mut self.view, delta)
    }
}

/// Perspective projection parameters
#[derive(Copy, Clone)]
pub struct Perspective {
    pub fov: f32,
    pub clip_near: f32,
    pub clip_far: f32,
}

/// Arcball camera parameters
#[derive(Copy, Clone)]
pub struct ArcBall {
    pub pivot: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

/// Arcball camera controller parameters
#[derive(Copy, Clone)]
pub struct ArcBallController {
    pub pan_sensitivity: f32,
    pub swivel_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub closest_zoom: f32,
}

impl Perspective {
    pub fn matrix(&self, width: f32, height: f32) -> Mat4 {
        Mat4::perspective_rh(width / height, self.fov, self.clip_near, self.clip_far)
    }
}

impl ArcBall {
    pub fn matrix(&self) -> Mat4 {
        Mat4::look_at_rh(
            self.pivot + self.eye(),
            self.pivot,
            Vec3::new(0.0, 1.0, 0.0),
        )
    }

    pub fn eye(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos().abs(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos().abs(),
        ) * self.distance
    }
}

impl ArcBallController {
    pub fn pivot(&mut self, arcball: &mut ArcBall, delta_x: f32, delta_y: f32) {
        arcball.yaw += delta_x * self.swivel_sensitivity;
        arcball.pitch += delta_y * self.swivel_sensitivity;

        arcball.pitch = arcball.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
    }

    pub fn pan(&mut self, arcball: &mut ArcBall, delta_x: f32, delta_y: f32, rate_z: f32) {
        let delta = Vec4::new(
            (-delta_x as f32) * arcball.distance,
            (delta_y as f32) * arcball.distance,
            0.0,
            0.0,
        ) * self.pan_sensitivity;

        // TODO: This is dumb, just use the cross product 4head
        let inv = arcball.matrix().inverse();
        let mut delta = (inv * delta).xyz();
        delta.z *= rate_z;
        arcball.pivot += delta;
    }

    pub fn zoom(&mut self, arcball: &mut ArcBall, delta: f32) {
        arcball.distance += delta * self.zoom_sensitivity.powf(2.) * arcball.distance;
        arcball.distance = arcball.distance.max(self.closest_zoom);
    }
}

impl Default for ArcBall {
    fn default() -> Self {
        Self {
            pivot: Vec3::ZERO,
            pitch: 0.3,
            yaw: -1.92,
            distance: 30.,
        }
    }
}

impl Default for Perspective {
    fn default() -> Self {
        Self {
            fov: 60.0f32.to_radians(),
            clip_near: 0.01,
            clip_far: 2_000.0,
        }
    }
}

impl Default for ArcBallController {
    fn default() -> Self {
        Self {
            pan_sensitivity: 0.0015,
            swivel_sensitivity: 0.005,
            zoom_sensitivity: 0.04,
            closest_zoom: 0.01,
        }
    }
}
