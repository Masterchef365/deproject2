use anyhow::{ensure, Ok, Result};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{path::Path, time::Duration};

use realsense_rust::{
    config::Config,
    context::Context,
    frame::PixelKind,
    frame::{ColorFrame, DepthFrame},
    kind::{Rs2CameraInfo, Rs2Format, Rs2StreamKind},
    pipeline::InactivePipeline,
};

use realsense_rust::{base::*, kind::Rs2DistortionModel};
use serde::{Deserialize, Serialize};

use crate::ImagePointCloud;

/// Ported from https://github.com/IntelRealSense/librealsense/blob/master/src/rs.cpp
/// Git rev 4e7050a
pub fn rs2_project_point_to_pixel(intrin: &Rs2Intrinsics, point: [f32; 3]) -> [f32; 2] {
    let mut x = point[0] / point[2];
    let mut y = point[1] / point[2];

    let distort = intrin.distortion();

    match distort.model {
        Rs2DistortionModel::BrownConradyModified | Rs2DistortionModel::BrownConradyInverse => {
            let r2 = x * x + y * y;
            let f = 1.
                + distort.coeffs[0] * r2
                + distort.coeffs[1] * r2 * r2
                + distort.coeffs[4] * r2 * r2 * r2;
            x *= f;
            y *= f;
            let dx = x + 2. * distort.coeffs[2] * x * y + distort.coeffs[3] * (r2 + 2. * x * x);
            let dy = y + 2. * distort.coeffs[3] * x * y + distort.coeffs[2] * (r2 + 2. * y * y);
            x = dx;
            y = dy;
        }

        Rs2DistortionModel::BrownConrady => {
            let r2 = x * x + y * y;
            let f = 1.
                + distort.coeffs[0] * r2
                + distort.coeffs[1] * r2 * r2
                + distort.coeffs[4] * r2 * r2 * r2;

            let xf = x * f;
            let yf = y * f;

            let dx = xf + 2. * distort.coeffs[2] * x * y + distort.coeffs[3] * (r2 + 2. * x * x);
            let dy = yf + 2. * distort.coeffs[3] * x * y + distort.coeffs[2] * (r2 + 2. * y * y);

            x = dx;
            y = dy;
        }

        Rs2DistortionModel::FThetaFisheye => {
            let mut r = (x * x + y * y).sqrt();
            if r < f32::EPSILON {
                r = f32::EPSILON;
            }
            let rd = 1.0 / distort.coeffs[0] * (2. * r * (distort.coeffs[0] / 2.0).tan()).atan();
            x *= rd / r;
            y *= rd / r;
        }

        Rs2DistortionModel::KannalaBrandt => {
            let mut r = (x * x + y * y).sqrt();
            if r < f32::EPSILON {
                r = f32::EPSILON;
            }
            let theta = r.atan();
            let theta2 = theta * theta;
            let series = 1.
                + theta2
                    * (distort.coeffs[0]
                        + theta2
                            * (distort.coeffs[1]
                                + theta2 * (distort.coeffs[2] + theta2 * distort.coeffs[3])));
            let rd = theta * series;
            x *= rd / r;
            y *= rd / r;
        }

        Rs2DistortionModel::None => (),
    }

    [
        x * intrin.fx() + intrin.ppx(),
        y * intrin.fy() + intrin.ppy(),
    ]
}

pub fn rs2_deproject_pixel_to_point(
    intrin: &Rs2Intrinsics,
    pixel: [f32; 2],
    depth: f32,
) -> [f32; 3] {
    //assert(intrin.model != RS2_DISTORTION_BROWN_CONRADY); // Cannot deproject to an brown conrady model

    let mut x = (pixel[0] - intrin.ppx()) / intrin.fx();
    let mut y = (pixel[1] - intrin.ppy()) / intrin.fy();

    let xo = x;
    let yo = y;

    let distort = intrin.distortion();

    match distort.model {
        Rs2DistortionModel::BrownConradyModified => {
            panic!("Deprojection does not support BrownConradyModified")
        }
        Rs2DistortionModel::BrownConradyInverse => {
            // need to loop until convergence
            // 10 iterations determined empirically
            for _ in 0..10 {
                let r2 = x * x + y * y;
                let icdist = 1.
                    / (1.
                        + ((distort.coeffs[4] * r2 + distort.coeffs[1]) * r2 + distort.coeffs[0])
                            * r2);
                let xq = x / icdist;
                let yq = y / icdist;
                let delta_x =
                    2. * distort.coeffs[2] * xq * yq + distort.coeffs[3] * (r2 + 2. * xq * xq);
                let delta_y =
                    2. * distort.coeffs[3] * xq * yq + distort.coeffs[2] * (r2 + 2. * yq * yq);
                x = (xo - delta_x) * icdist;
                y = (yo - delta_y) * icdist;
            }
        }
        Rs2DistortionModel::BrownConrady => {
            // need to loop until convergence
            // 10 iterations determined empirically
            for _ in 0..10 {
                let r2 = x * x + y * y;
                let icdist = 1.
                    / (1.
                        + ((distort.coeffs[4] * r2 + distort.coeffs[1]) * r2 + distort.coeffs[0])
                            * r2);
                let delta_x =
                    2. * distort.coeffs[2] * x * y + distort.coeffs[3] * (r2 + 2. * x * x);
                let delta_y =
                    2. * distort.coeffs[3] * x * y + distort.coeffs[2] * (r2 + 2. * y * y);
                x = (xo - delta_x) * icdist;
                y = (yo - delta_y) * icdist;
            }
        }
        Rs2DistortionModel::KannalaBrandt => {
            let mut rd = (x * x + y * y).sqrt();
            if rd < f32::EPSILON {
                rd = f32::EPSILON;
            }

            let mut theta = rd;
            let mut theta2 = rd * rd;
            for _ in 0..4 {
                let f = theta
                    * (1.
                        + theta2
                            * (distort.coeffs[0]
                                + theta2
                                    * (distort.coeffs[1]
                                        + theta2
                                            * (distort.coeffs[2] + theta2 * distort.coeffs[3]))))
                    - rd;
                if f.abs() < f32::EPSILON {
                    break;
                }
                let df = 1.
                    + theta2
                        * (3. * distort.coeffs[0]
                            + theta2
                                * (5. * distort.coeffs[1]
                                    + theta2
                                        * (7. * distort.coeffs[2]
                                            + 9. * theta2 * distort.coeffs[3])));
                theta -= f / df;
                theta2 = theta * theta;
            }
            let r = (theta).tan();
            x *= r / rd;
            y *= r / rd;
        }
        Rs2DistortionModel::FThetaFisheye => {
            let mut rd = (x * x + y * y).sqrt();
            if rd < f32::EPSILON {
                rd = f32::EPSILON;
            }
            let r = (distort.coeffs[0] * rd).tan() / (2. * (distort.coeffs[0] / 2.0).tan()).atan();
            x *= r / rd;
            y *= r / rd;
        }
        Rs2DistortionModel::None => (),
    }

    [depth * x, depth * y, depth]
}

pub fn rs2_transform_point_to_point(extrin: &Rs2Extrinsics, from_point: [f32; 3]) -> [f32; 3] {
    let rot = extrin.rotation();
    let tl = extrin.translation();
    [
        rot[0] * from_point[0] + rot[3] * from_point[1] + rot[6] * from_point[2] + tl[0],
        rot[1] * from_point[0] + rot[4] * from_point[1] + rot[7] * from_point[2] + tl[1],
        rot[2] * from_point[0] + rot[5] * from_point[1] + rot[8] * from_point[2] + tl[2],
    ]
}

pub fn align_images(
    depth_intrin: &Rs2Intrinsics,
    depth_to_other: &Rs2Extrinsics,
    other_intrin: &Rs2Intrinsics,
    depth: &[u16],
    input_img: &[[u8; 3]],
    output_img: &mut [[u8; 3]],
) {
    // Iterate over the pixels of the depth image
    let depth_width = depth_intrin.width();
    let color_width = other_intrin.width();
    let color_height = other_intrin.height();

    for depth_y in 0..depth_intrin.height() {
        //let mut depth_pixel_index = depth_y * depth_intrin.width();
        for depth_x in 0..depth_intrin.width() {
            let depth_pixel_index = depth_y * depth_width + depth_x;

            // Skip over depth pixels with the value of zero, we have no depth data so we will not write anything into our aligned images
            let depth = depth[depth_pixel_index];
            if depth == 0 {
                continue;
            }

            // Map the top-left corner of the depth pixel onto the other image
            let depth_pixel = [depth_x as f32 - 0.5, depth_y as f32 - 0.5];
            let depth_point =
                rs2_deproject_pixel_to_point(depth_intrin, depth_pixel, f32::from(depth));
            let other_point = rs2_transform_point_to_point(depth_to_other, depth_point);
            let other_pixel = rs2_project_point_to_pixel(other_intrin, other_point);
            let other_x0 = (other_pixel[0] + 0.5) as i32;
            let other_y0 = (other_pixel[1] + 0.5) as i32;

            // Map the bottom-right corner of the depth pixel onto the other image
            let depth_pixel = [depth_x as f32 + 0.5, depth_y as f32 + 0.5];
            let depth_point =
                rs2_deproject_pixel_to_point(depth_intrin, depth_pixel, f32::from(depth));
            let other_point = rs2_transform_point_to_point(depth_to_other, depth_point);
            let other_pixel = rs2_project_point_to_pixel(other_intrin, other_point);
            let other_x1 = (other_pixel[0] + 0.5) as i32;
            let other_y1 = (other_pixel[1] + 0.5) as i32;

            if other_x0 < 0
                || other_y0 < 0
                || other_x1 >= color_width as i32
                || other_y1 >= color_height as i32
            {
                continue;
            }

            // Transfer between the depth pixels and the pixels inside the rectangle on the other image
            for y in other_y0..=other_y1 {
                for x in other_x0..=other_x1 {
                    let x = x as usize;
                    let y = y as usize;
                    output_img[depth_pixel_index] = input_img[y * color_width + x];
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Rs2IntrinsicsSerde {
    /// Width of the image in pixels"]
    pub width: i32,
    /// Height of the image in pixels"]
    pub height: i32,
    /// Horizontal coordinate of the principal point of the image, as a pixel offset from the left edge"]
    pub ppx: f32,
    /// Vertical coordinate of the principal point of the image, as a pixel offset from the top edge"]
    pub ppy: f32,
    /// Focal length of the image plane, as a multiple of pixel width"]
    pub fx: f32,
    /// Focal length of the image plane, as a multiple of pixel height"]
    pub fy: f32,
    /// Distortion model of the image"]
    pub model: u32,
    /// Distortion coefficients. Order for Brown-Conrady: [k1, k2, p1, p2, k3]. Order for F-Theta Fish-eye: [k1, k2, k3, k4, 0]. Other models are subject to their own interpretations"]
    pub coeffs: [f32; 5usize],
}

impl Into<realsense_sys::rs2_intrinsics> for Rs2IntrinsicsSerde {
    fn into(self) -> realsense_sys::rs2_intrinsics {
        realsense_sys::rs2_intrinsics {
            width: self.width,
            height: self.height,
            ppx: self.ppx,
            ppy: self.ppy,
            fx: self.fx,
            fy: self.fy,
            model: self.model,
            coeffs: self.coeffs,
        }
    }
}

impl From<realsense_sys::rs2_intrinsics> for Rs2IntrinsicsSerde {
    fn from(r: realsense_sys::rs2_intrinsics) -> Self {
        Self {
            width: r.width,
            height: r.height,
            ppx: r.ppx,
            ppy: r.ppy,
            fx: r.fx,
            fy: r.fy,
            model: r.model,
            coeffs: r.coeffs,
        }
    }
}

/// Gets frames from the realsense, processes them, and then calls "callback". Intended to be
/// embedded in an external thread, since this method never returns
pub fn realsense_mainloop(mut callback: impl FnMut(ImagePointCloud)) -> Result<()> {
    // Check for depth or color-compatible devices.
    let queried_devices = HashSet::new(); // Query any devices
    let context = Context::new()?;
    let devices = context.query_devices(queried_devices);
    ensure!(!devices.is_empty(), "No devices found");

    let device = &devices[0];

    // TODO: Support devices other than the D415!
    // Create pipeline
    let pipeline = InactivePipeline::try_from(&context)?;
    let mut config = Config::new();
    config
        .enable_device_from_serial(device.info(Rs2CameraInfo::SerialNumber).unwrap())?
        .disable_all_streams()?
        .enable_stream(Rs2StreamKind::Color, None, 640, 0, Rs2Format::Bgr8, 15)?
        .enable_stream(Rs2StreamKind::Depth, None, 640, 0, Rs2Format::Z16, 15)
        .unwrap();

    // Change pipeline's type from InactivePipeline -> ActivePipeline
    let mut pipeline = pipeline.start(Some(config))?;

    let streams = pipeline.profile().streams();

    let depth_stream = streams
        .iter()
        .find(|p| p.kind() == Rs2StreamKind::Depth)
        .unwrap();
    let color_stream = streams
        .iter()
        .find(|p| p.kind() == Rs2StreamKind::Color)
        .unwrap();

    let depth_intrinsics = depth_stream.intrinsics()?;
    let depth_to_color_extrinsics = depth_stream.extrinsics(color_stream)?;
    let color_intrinsics = color_stream.intrinsics()?;

    let mut in_color_buf: Vec<[u8; 3]> = vec![];
    let mut in_depth_buf: Vec<u16> = vec![];
    let mut out_color_buf: Vec<[u8; 3]> = vec![];

    let timeout = Duration::from_millis(2000);
    loop {
        let frames = pipeline.wait(Some(timeout)).unwrap();
        let color_frame: &ColorFrame = &frames.frames_of_type()[0];
        let depth_frame: &DepthFrame = &frames.frames_of_type()[0];

        in_depth_buf.clear();
        in_color_buf.clear();
        out_color_buf.clear();

        in_depth_buf.extend(depth_frame.iter().map(|p| match p {
            PixelKind::Z16 { depth } => depth,
            _ => panic!("{:?}", p),
        }));

        in_color_buf.extend(color_frame.iter().map(|p| match p {
            PixelKind::Bgr8 { b, g, r } => [*r, *g, *b],
            _ => panic!("{:?}", p),
        }));

        out_color_buf.resize(in_depth_buf.len(), [0; 3]);

        align_images(
            &depth_intrinsics,
            &depth_to_color_extrinsics,
            &color_intrinsics,
            &in_depth_buf,
            &in_color_buf,
            &mut out_color_buf,
        );

        // Convert for use elsewhere
        let valid = in_depth_buf.iter().map(|depth| *depth != 0).collect();
        let mut position = vec![];
        let (width, height) = (color_frame.width(), color_frame.height());
        for y in 0..height {
            for x in 0..width {
                let pixel_idx = y * width + x;
                let pt = rs2_deproject_pixel_to_point(
                    &depth_intrinsics,
                    [x as f32 - 0.5, y as f32 - 0.5],
                    in_depth_buf[pixel_idx] as f32,
                );
                //let pt = pt.map(|v| v / 1e1);
                position.push(pt.into());
            }
        }

        let pcld_data = ImagePointCloud::new(valid, position, out_color_buf.clone(), width);

        callback(pcld_data);
    }
}
