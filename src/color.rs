use palette::LinSrgba;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Color(pub LinSrgba<f64>);

impl Color {}

impl From<egui::Color32> for Color {
    fn from(value: egui::Color32) -> Self {
        let x = LinSrgba::from_components(value.to_tuple());
        Self(x.into_format())
    }
}

impl From<Color> for egui::Color32 {
    fn from(value: Color) -> Self {
        let x = value.0;
        let x = x.into_format();
        Self::from_rgba_premultiplied(x.red, x.green, x.blue, x.alpha)
    }
}

impl From<Color> for wgpu::Color {
    fn from(value: Color) -> Self {
        let x = value.0;
        Self {
            r: x.red,
            g: x.green,
            b: x.blue,
            a: x.alpha,
        }
    }
}

impl From<wgpu::Color> for Color {
    fn from(c: wgpu::Color) -> Self {
        Self(LinSrgba::from_components((c.r, c.g, c.b, c.a)))
    }
}

impl From<Color> for [f32; 4] {
    fn from(value: Color) -> Self {
        let c: LinSrgba<f32> = value.0.into_format();
        [c.red, c.green, c.blue, c.alpha]
    }
}

impl From<Color> for [f64; 4] {
    fn from(value: Color) -> Self {
        let c = value.0;
        [c.red, c.green, c.blue, c.alpha]
    }
}
