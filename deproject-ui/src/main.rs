use deproject_io::{realsense_mainloop, ImagePointCloud};
use eframe::{
    egui::{self, Context, DragValue, SidePanel, Ui},
    epaint::Vec2,
};
use egui::mutex::Mutex;
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc,
};
use view3d::{RenderMsg, Viewport3d, ViewportState};

mod camera;
mod shapes;
mod vertex;
mod view3d;
use vertex::Vertex;

#[derive(PartialEq)]
enum Tabs {
    Record,
    Calibrate,
}

struct MyApp {
    view3d: Arc<Mutex<Viewport3d>>,
    viewport_state: ViewportState,
    cfg: AppConfig,
    render_tx: Sender<RenderMsg>,
    camera_rx: Receiver<ImagePointCloud>,
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
        //initial_window_size: Some(egui::vec2(350.0, 380.0)),
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
        let (render_tx, rx) = channel();

        render_tx
            .send(RenderMsg {
                lines: shapes::default_grid(),
                points: vec![],
            })
            .unwrap();

        let view3d = Viewport3d::new(&gl, rx);

        let camera_rx = spawn_realsense_thread();

        Self {
            camera_rx,
            viewport_state: ViewportState::default(),
            view3d: Arc::new(Mutex::new(view3d)),
            render_tx,
            cfg: AppConfig::default(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("Left").show(ctx, |ui| {
            app_ui(ui, &mut self.cfg);
        });

        if let Some(latest_frame) = self.camera_rx.try_iter().last() {
            let pointcloud = latest_frame
                .iter_pixels()
                .filter_map(|x| x)
                .map(|(pos, color)| Vertex::new(pos.into(), color.map(|c| c as f32 / 256.0)))
                .collect();
            self.render_tx
                .send(RenderMsg {
                    points: pointcloud,
                    lines: vec![],
                })
                .unwrap();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                //self.show_calibration_pattern(ui);
                view3d::viewport_widget(&mut self.viewport_state, self.view3d.clone(), ui);
            });
        });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(_gl) = gl {
            // TODO: Destroy viewport3d here?
        }
    }
}

fn spawn_realsense_thread() -> Receiver<ImagePointCloud> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let callback = |x| tx.send(x).unwrap();
        realsense_mainloop(callback).unwrap();
    });
    rx
}
