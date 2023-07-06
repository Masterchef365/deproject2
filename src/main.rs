use eframe::egui::{self, SidePanel, Ui};

#[derive(PartialEq)]
enum Tabs {
    Record,
    Calibrate,
}

fn main() -> eframe::Result<()> {
    let mut tab = Tabs::Record;

    let options = eframe::NativeOptions::default();
    eframe::run_simple_native("My egui App", options, move |ctx, _frame| {
        SidePanel::left("Left").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut tab, Tabs::Record, "Record");
                ui.selectable_value(&mut tab, Tabs::Calibrate, "Calibrate");
            });
        });
    })
}
