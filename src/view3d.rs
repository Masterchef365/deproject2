use crate::{camera::Camera, Vertex};
use eframe::{egui, emath::Vec2};
use egui::mutex::Mutex;
use glow::HasContext;
use glow::VERTEX_PROGRAM_POINT_SIZE;
use std::sync::{mpsc::Receiver, Arc};

#[derive(Default, Clone)]
pub struct RenderMsg {
    pub lines: Vec<Vertex>,
    pub points: Vec<Vertex>,
}

pub struct Viewport3d {
    program: glow::Program,

    point_array: glow::VertexArray,
    point_buf: glow::NativeBuffer,
    point_count: i32,

    line_array: glow::VertexArray,
    line_buf: glow::NativeBuffer,
    line_count: i32,

    rx: Receiver<RenderMsg>,
}

#[derive(Clone)]
pub struct ViewportState {
    pub camera: Camera,
    pub spread: f32,
    pub point_size: f32,
}

pub fn viewport_widget(
    state: &mut ViewportState,
    view3d: Arc<Mutex<Viewport3d>>,
    ui: &mut egui::Ui,
) {
    let space = ui.available_size(); //Vec2::splat(ui.available_size().min_elem());

    let (rect, response) = ui.allocate_exact_size(space, egui::Sense::drag());

    // Camera movement
    if response.dragged_by(egui::PointerButton::Primary) {
        if ui.input(|i| i.raw.modifiers.shift_only()) {
            state.camera.pan(
                response.drag_delta().x,
                response.drag_delta().y,
                state.spread.powi(-2),
            );
        } else {
            state
                .camera
                .pivot(response.drag_delta().x, response.drag_delta().y);
        }
    }

    if response.dragged_by(egui::PointerButton::Secondary) {
        state.camera.pan(
            response.drag_delta().x,
            response.drag_delta().y,
            state.spread.powi(-2),
        );
    }

    if response.hovered() {
        state.camera.zoom(ui.input(|i| i.scroll_delta.y));
    }

    // Clone locals so we can move them into the paint callback:
    let state = state.clone();

    let callback = egui::PaintCallback {
        rect,
        callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
            view3d.lock().paint(painter.gl(), state.clone(), space);
        })),
    };
    ui.painter().add(callback);
}

impl Viewport3d {
    pub fn new(gl: &glow::Context, rx: Receiver<RenderMsg>) -> Self {
        use glow::HasContext as _;

        // Compile shaders
        let shader_version = if cfg!(target_arch = "wasm32") {
            "#version 310 es"
        } else {
            "#version 430"
        };

        unsafe {
            // TODO: Su
            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("shaders/pointcloud.vert")),
                (
                    glow::FRAGMENT_SHADER,
                    include_str!("shaders/pointcloud.frag"),
                ),
            ];

            let program = compile_glsl_program(gl, &shader_sources).unwrap();

            // Create pointcloud buffer
            let point_array = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(point_array));
            let point_verts = vec![Vertex::new([0., 0., 0.], [0., 0., 0.])];
            let point_count = point_verts.len() as i32;
            let point_buf = gl.create_buffer().expect("Cannot create vertex buffer");
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(point_buf));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&point_verts),
                glow::STREAM_DRAW,
            );

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            // Create line buffer
            let line_array = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(line_array));
            let line_verts = vec![Vertex::new([0.; 3], [0.; 3]); 50];
            let line_count = line_verts.len() as i32;
            let line_buf = gl.create_buffer().expect("Cannot create vertex buffer");
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(line_buf));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&line_verts),
                glow::STREAM_DRAW,
            );

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            for (array, buf) in [(line_array, line_buf), (point_array, point_buf)] {
                gl.bind_vertex_array(Some(array));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(buf));

                // Set vertex attributes
                gl.enable_vertex_attrib_array(0);
                gl.vertex_attrib_pointer_f32(
                    0,
                    3,
                    glow::FLOAT,
                    false,
                    std::mem::size_of::<Vertex>() as i32,
                    0,
                );

                gl.enable_vertex_attrib_array(1);
                gl.vertex_attrib_pointer_f32(
                    1,
                    3,
                    glow::FLOAT,
                    false,
                    std::mem::size_of::<Vertex>() as i32,
                    3 * std::mem::size_of::<f32>() as i32,
                );

                gl.bind_vertex_array(None);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }

            Self {
                program,

                point_array,
                point_buf,
                point_count,

                line_array,
                line_buf,
                line_count,

                rx,
            }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.point_array);
            gl.delete_vertex_array(self.line_array);
        }
    }

    fn paint(&mut self, gl: &glow::Context, state: ViewportState, size: Vec2) {
        use glow::HasContext as _;

        unsafe {
            // Upload any new geometry
            if let Some(msg) = self.rx.try_iter().last() {
                let RenderMsg { lines, points } = msg;
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.line_buf));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&lines),
                    glow::STREAM_DRAW,
                );
                self.line_count = lines.len() as i32;

                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.point_buf));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&points),
                    glow::STREAM_DRAW,
                );
                self.point_count = points.len() as i32;
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }

            // Enable depth buffer (disabled by egui each frame)
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.depth_range_f32(0., 1.);

            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            // Draw points
            gl.use_program(Some(self.program));

            let view = state.camera.view();
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "u_view").as_ref(),
                false,
                bytemuck::cast_slice(view.as_ref()),
            );

            let projection = state.camera.projection(size.x, size.y);
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "u_projection")
                    .as_ref(),
                false,
                bytemuck::cast_slice(projection.as_ref()),
            );

            gl.uniform_2_f32(
                gl.get_uniform_location(self.program, "u_spread").as_ref(),
                state.camera.view.pivot.z,
                state.spread.powi(2),
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "u_ptsize").as_ref(),
                state.point_size,
            );

            gl.enable(VERTEX_PROGRAM_POINT_SIZE);

            gl.bind_vertex_array(None);
            gl.bind_vertex_array(Some(self.point_array));
            gl.draw_arrays(glow::POINTS, 0, self.point_count);

            gl.bind_vertex_array(None);
            gl.bind_vertex_array(Some(self.line_array));
            gl.draw_arrays(glow::LINES, 0, self.line_count);
        }
    }
}

impl RenderMsg {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn append(&mut self, other: &RenderMsg) {
        self.lines.extend_from_slice(&other.lines);
        self.points.extend_from_slice(&other.points);
    }
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            spread: 1.0,
            point_size: 2.0,
        }
    }
}

fn compile_glsl_program(
    gl: &glow::Context,
    sources: &[(u32, &str)],
) -> Result<glow::Program, String> {
    // Compile default shaders
    unsafe {
        let program = gl.create_program()?;

        let mut shaders = vec![];

        for (stage, shader_source) in sources {
            let shader = gl.create_shader(*stage)?;

            gl.shader_source(shader, shader_source);

            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                return Err(format!(
                    "OpenGL compile shader: {}",
                    gl.get_shader_info_log(shader)
                ));
            }

            gl.attach_shader(program, shader);

            shaders.push(shader);
        }

        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            return Err(format!(
                "OpenGL link shader: {}",
                gl.get_program_info_log(program)
            ));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        Ok(program)
    }
}
