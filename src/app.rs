use egui::Color32;
use winit::{event::WindowEvent, window::WindowId};

use crate::color::Color;

pub struct App {
    pub bg_color: Color,
    pub triangle_color: Color,
    pub blur_kernel: u8,
}

impl App {
    pub fn new() -> Self {
        Self {
            bg_color: Color(palette::LinSrgba::from_components((0.1, 0.1, 0.1, 1.0))),
            triangle_color: Color32::BLUE.into(),
            blur_kernel: 0,
        }
    }

    pub fn handle_window_event(&mut self, _window_id: WindowId, _event: &WindowEvent) {}

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("stuff")
            .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
            // .open(&mut self.open)
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                egui::Grid::new("my_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("triangle color");
                        let mut color = self.triangle_color.into();
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            self.triangle_color = color.into();
                        };
                        ui.end_row();

                        ui.label("bg color");
                        let mut color = self.bg_color.into();
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            self.bg_color = color.into();
                        };
                        ui.end_row();

                        ui.label("Blur Kernel Size");
                        ui.add(egui::DragValue::new(&mut self.blur_kernel).clamp_range(0..=120));
                        ui.end_row();
                    });
            });
    }
}
