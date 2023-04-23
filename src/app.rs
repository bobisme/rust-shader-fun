use egui::Color32;
use winit::{event::WindowEvent, window::WindowId};

pub struct App {
    pub triangle_color: Color32,
    color_picker_open: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            triangle_color: Color32::BLUE,
            color_picker_open: true,
        }
    }

    pub fn handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) {
        // You can handle window events here, e.g., keyboard input or resizing.
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left(egui::Id::new("Controls")).show(ctx, |ui| {
            // egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::Label::new("Select the triangle color"));
            ui.color_edit_button_srgba(&mut self.triangle_color);
        });
    }
}
