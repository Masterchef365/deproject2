use eframe::egui::{self, Context, DragValue, SidePanel, Ui};

#[derive(PartialEq)]
enum Tabs {
    Record,
    Calibrate,
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

fn main() -> eframe::Result<()> {
    let mut cfg = AppConfig::default();

    let options = eframe::NativeOptions::default();
    eframe::run_simple_native("Deproject calibrator", options, move |ctx, frame| {
        frame.set_fullscreen(true);
        SidePanel::left("Left").show(ctx, |ui| app_ui(ui, &mut cfg));
    })
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
    ui.label("Controls the granularity of the calibration pattern displayed by the projector, in powers of 2. Optimally, this should be the resolution of the projector.");
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
    let pixels = ctx.pixels_per_point() * ctx.screen_rect().size();
    (pixels.x.log2().ceil() as _, pixels.y.log2().ceil() as _)
}

impl Default for CalibratorConfig {
    fn default() -> Self {
        Self {}
    }
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            horiz_subdivs: 12,
            vert_subdivs: 11,
            pics_per_pattern: 1,
        }
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::Record
    }
}
