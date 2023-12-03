use glam::Vec3;

mod realsense;

pub use realsense::realsense_mainloop;

#[derive(Default)]
pub struct ImagePointCloud {
    valid: Vec<bool>,
    position: Vec<Vec3>,
    color: Vec<[u8; 3]>,
    width: usize,
}

/// 3D position relative to camera, RGB color
pub type Sample = (Vec3, [u8; 3]);

impl ImagePointCloud {
    pub fn new(valid: Vec<bool>, position: Vec<Vec3>, color: Vec<[u8; 3]>, width: usize) -> Self {
        assert_eq!(valid.len(), position.len());
        assert_eq!(valid.len(), color.len());
        assert_eq!(valid.len() % width, 0);

        Self {
            valid,
            position,
            color,
            width,
        }
    }

    /// Returns a sample for each pixel
    pub fn iter_pixels(&self) -> impl Iterator<Item = Option<Sample>> + '_ {
        self.position
            .iter()
            .copied()
            .zip(self.color.iter().copied())
            .zip(&self.valid)
            .map(|(pos_color, valid)| valid.then(|| pos_color))
    }

    /// Pixel dimension height
    pub fn height(&self) -> usize {
        self.valid.len() / self.width
    }

    /// Pixel dimension width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Position data (subject to `valid` array)
    pub fn position(&self) -> &[Vec3] {
        &self.position
    }

    /// Color data (subject to `valid` array)
    pub fn color(&self) -> &[[u8; 3]] {
        &self.color
    }

    /// Whether each pixel is valid data
    pub fn valid(&self) -> &[bool] {
        &self.valid
    }
}
