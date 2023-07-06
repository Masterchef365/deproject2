use eframe::{egui::{self, Context, DragValue, SidePanel, Ui}, epaint::Vec2};
use egui::mutex::Mutex;
use std::sync::Arc;

#[derive(PartialEq)]
enum Tabs {
    Record,
    Calibrate,
}

struct MyApp {
    pattern: Arc<Mutex<ProjectorPatternPainter>>,
    cfg: AppConfig,
}

#[derive(Default)]
struct AppConfig {
    calib: CalibratorConfig,
    record: RecorderConfig,
    tab: Tabs,
}

struct CalibratorConfig {}

struct RecorderConfig {
    /// Number of horizontal subdivisions, pixel resolution is 2**n
    horiz_subdivs: usize,
    /// Number of vertical subdivisions, pixel resolution is 2**v
    vert_subdivs: usize,
    /// Number of frames to capture for each pattern
    pics_per_pattern: usize,
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(350.0, 380.0)),
        multisampling: 4,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Custom 3D painting in eframe using glow",
        options,
        Box::new(|cc| Box::new(MyApp::new(cc))),
    )
}

fn app_ui(ui: &mut Ui, state: &mut AppConfig) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.tab, Tabs::Record, "Record");
        ui.selectable_value(&mut state.tab, Tabs::Calibrate, "Calibrate");
    });

    if state.tab == Tabs::Record {
        record_ui(ui, &mut state.record);
    }

    if state.tab == Tabs::Calibrate {
        calib_ui(ui, &mut state.calib);
    }
}

fn record_ui(ui: &mut Ui, state: &mut RecorderConfig) {
    // Subdivision
    ui.strong("Subdivisions");
    ui.label("Controls the granularity of the calibration pattern displayed by the projector, in powers of 2. This should be close to the resolution of the projector.");
    ui.add(
        DragValue::new(&mut state.horiz_subdivs)
            .prefix("Horizontal resolution: ")
            .custom_formatter(|x, _| 2_u64.pow(x as _).to_string())
            .speed(2e-2)
            .clamp_range(1..=25),
    );
    ui.add(
        DragValue::new(&mut state.vert_subdivs)
            .prefix("Vertical subdivs: ")
            .custom_formatter(|x, _| 2_u64.pow(x as _).to_string())
            .speed(2e-2)
            .clamp_range(1..=25),
    );

    if ui.button("Fit to window size").clicked() {
        let (h, v) = fit_subdivs_to_window(ui.ctx());
        state.vert_subdivs = v;
        state.horiz_subdivs = h;
    }

    ui.separator();

    // Capture
    ui.strong("Capture");
    ui.add(
        DragValue::new(&mut state.pics_per_pattern)
            .prefix("Frames per pattern: ")
            .clamp_range(1..=15),
    );

    ui.centered_and_justified(|ui| {
        if ui.button("Start").clicked() {
            todo!()
        }
    });
}

fn calib_ui(ui: &mut Ui, state: &mut CalibratorConfig) {
    let c = ui.ctx().clone();
    c.inspection_ui(ui);
}

/// Returns the number of horizontal and vertical subdivisions to use for this window
fn fit_subdivs_to_window(ctx: &Context) -> (usize, usize) {
    let pixels = window_size_in_pixels(ctx);
    (pixels.x.log2().ceil() as _, pixels.y.log2().ceil() as _)
}

fn window_size_in_pixels(ctx: &Context) -> Vec2 {
    ctx.pixels_per_point() * ctx.screen_rect().size()
}

impl Default for CalibratorConfig {
    fn default() -> Self {
        Self {}
    }
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            horiz_subdivs: 11,
            vert_subdivs: 10,
            pics_per_pattern: 1,
        }
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::Record
    }
}


impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");
        Self {
            pattern: Arc::new(Mutex::new(ProjectorPatternPainter::new(gl))),
            cfg: AppConfig::default(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("Left").show(ctx, |ui| {
            app_ui(ui, &mut self.cfg);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.custom_painting(ui);
        });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.pattern.lock().destroy(gl);
        }
    }
}

impl MyApp {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (rect, _response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());

        // Clone locals so we can move them into the paint callback:
        let pattern = self.pattern.clone();

        let window_size = window_size_in_pixels(ui.ctx());

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                pattern.lock().paint(painter.gl(), window_size);
            })),
        };
        ui.painter().add(callback);
    }
}

struct ProjectorPatternPainter {
    program: glow::Program,
    vertex_array: glow::VertexArray,
}

impl ProjectorPatternPainter {
    fn new(gl: &glow::Context) -> Self {
        use glow::HasContext as _;

        let shader_version = if cfg!(target_arch = "wasm32") {
            "#version 300 es"
        } else {
            "#version 330"
        };

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                r#"
                    // https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/
                    out vec2 uv;
                    
                    void main() {
                        uv = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
                        gl_Position = vec4(uv * 2.0f + -1.0f, 0.0f, 1.0f);
                    }
                "#,
                r#"
                    precision mediump float;
                    in vec2 uv;
                    out vec4 out_color;
                    void main() {
                        out_color = vec4(uv, 0, 1);
                    }
                "#,
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(shader, &format!("{}\n{}", shader_version, shader_source));
                    gl.compile_shader(shader);
                    assert!(
                        gl.get_shader_compile_status(shader),
                        "Failed to compile {shader_type}: {}",
                        gl.get_shader_info_log(shader)
                    );
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            assert!(
                gl.get_program_link_status(program),
                "{}",
                gl.get_program_info_log(program)
            );

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");

            Self {
                program,
                vertex_array,
            }
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vertex_array);
        }
    }

    fn paint(&self, gl: &glow::Context, size: Vec2) {
        use glow::HasContext as _;
        unsafe {
            // Take up the whole screen!
            gl.scissor(0, 0, size.x as _, size.y as _);
            gl.viewport(0, 0, size.x as _, size.y as _);

            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }
}
