use eframe::{
    egui::{self, TextureOptions, Ui},
    epaint::{ColorImage, ImageData, ImageDelta, TextureId, Vec2},
};
use ndarray::*;
use vorpal_core::ndarray;

use crate::node_editor::*;

// ========= First, define your user data types =============

pub struct NodeGraphExample {
    nodes: NodeGraphWidget,
    image: ImageViewWidget,
    data: Array3<f32>,
}

impl Default for NodeGraphExample {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            image: Default::default(),
            data: Array3::zeros((100, 100, 3)),
        }
    }
}

#[cfg(feature = "persistence")]
const PERSISTENCE_KEY: &str = "egui_node_graph";

#[cfg(feature = "persistence")]
impl NodeGraphExample {
    /// If the persistence feature is enabled, Called once before the first frame.
    /// Load previous app state (if any).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let state = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, PERSISTENCE_KEY))
            .unwrap_or_default();
        Self {
            state,
            user_state: MyGraphState::default(),
        }
    }
}

impl eframe::App for NodeGraphExample {
    #[cfg(feature = "persistence")]
    /// If the persistence function is enabled,
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PERSISTENCE_KEY, &self.state);
    }
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.image
            .set_image("my image".into(), ctx, array_to_imagedata(&self.data));

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });
        egui::SidePanel::left("yeahhhh").show(ctx, |ui| {
            self.nodes.show(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.image.show(ui);
        });
    }
}

#[derive(Default)]
struct ImageViewWidget {
    tex: Option<TextureId>,
}

impl ImageViewWidget {
    const OPTS: TextureOptions = TextureOptions::NEAREST;

    fn show(&mut self, ui: &mut Ui) {
        if let Some(tex) = self.tex {
            ui.image(tex, ui.available_size());
        }
    }

    fn set_image(&mut self, name: String, ctx: &egui::Context, image: ImageData) {
        if let Some(tex) = self.tex {
            ctx.tex_manager()
                .write()
                .set(tex, ImageDelta::full(image, Self::OPTS))
        } else {
            self.tex = Some(ctx.tex_manager().write().alloc(name, image, Self::OPTS))
        }
    }
}

/// Converts an image of 0 - 1 flaots into egui image data
pub fn array_to_imagedata(array: &ndarray::Array3<f32>) -> ImageData {
    assert_eq!(array.shape()[2], 3, "Image must be RGB");
    assert!(array.len() > 0);
    let dims = [array.shape()[0], array.shape()[1]];
    let rgb: Vec<u8> = array
        .iter()
        .map(|value| (value.clamp(0., 1.) * 255.0) as u8)
        .collect();
    ImageData::Color(ColorImage::from_rgb(dims, &rgb))
}
