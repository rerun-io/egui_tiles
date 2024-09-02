#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct PlotDemo {}

impl PlotDemo {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Hello egui_table!");
        _ = self;
    }
}
