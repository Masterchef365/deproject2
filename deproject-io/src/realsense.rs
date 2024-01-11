use anyhow::{ensure, Ok, Result};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;
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

use crate::realsense_utils::*;

/// Gets frames from the realsense, processes them, and then calls "callback". Intended to be
/// embedded in an external thread, since this method never returns
pub fn realsense_mainloop(mut callback: impl FnMut(ImagePointCloud), color_width: usize, color_height: usize, depth_width: usize, depth_height: usize, fps: usize) -> Result<()> {
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
        .enable_stream(Rs2StreamKind::Color, None, color_width, color_height, Rs2Format::Bgr8, fps)?
        .enable_stream(Rs2StreamKind::Depth, None, depth_width, depth_height, Rs2Format::Z16, fps)
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

    let mut last_elap = Instant::now();

    let timeout = Duration::from_millis(2000);
    loop {
        let fps = 1. / last_elap.elapsed().as_secs_f32();
        println!("FPS: {fps}");

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
