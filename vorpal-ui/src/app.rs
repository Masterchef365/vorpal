use std::time::Instant;

use eframe::{
    egui::{self, TextureOptions, Ui},
    epaint::{ColorImage, ImageData, ImageDelta, TextureId, Vec2},
};
use ndarray::*;
use vorpal_core::{ndarray, ExternInputId, Value};

use crate::node_editor::*;

// ========= First, define your user data types =============

pub struct NodeGraphExample {
    nodes: NodeGraphWidget,
    image: ImageViewWidget,
    data: NdArray<f32>,
    time: Instant,
}

const TIME_KEY: &str = "Time (seconds)";
const POS_KEY: &str = "Position (pixels)";
const RESOLUTION_KEY: &str = "Resolution (pixels)";

impl Default for NodeGraphExample {
    fn default() -> Self {
        let mut nodes = NodeGraphWidget::default();
        nodes.context_mut().insert_input(
            &ExternInputId::new(TIME_KEY.to_string()),
            Value::Scalar(0.1),
        );
        nodes.context_mut().insert_input(
            &ExternInputId::new(POS_KEY.to_string()),
            Value::Vec2([0.; 2]),
        );
        nodes.context_mut().insert_input(
            &ExternInputId::new(RESOLUTION_KEY.to_string()),
            Value::Vec2([1.; 2]),
        );

        Self {
            time: Instant::now(),
            nodes,
            image: Default::default(),
            data: NdArray::zeros(vec![100, 100, 3]),
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
        ctx.request_repaint();

        for i in 0..self.data.shape()[0] {
            for j in 0..self.data.shape()[1] {
                self.nodes.context_mut().insert_input(
                    &ExternInputId::new(POS_KEY.into()),
                    Value::Vec2([i as f32, j as f32]),
                );

                let Ok(Value::Vec4(result)) = self.nodes.eval_output_node() else {
                    panic!("Failed to eval node");
                };

                for k in 0..self.data.shape()[2] {
                    self.data[[i, j, k]] = result[k];
                }
            }
        }

        self.image
            .set_image("my image".into(), ctx, array_to_imagedata(&self.data));

        self.nodes.context_mut().insert_input(
            &ExternInputId::new(TIME_KEY.to_string()),
            Value::Scalar(self.time.elapsed().as_secs_f32()),
        );

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
pub fn array_to_imagedata(array: &ndarray::NdArray<f32>) -> ImageData {
    assert_eq!(
        array.shape().len(),
        3,
        "Array must have shape [width, height, 3]"
    );
    assert_eq!(array.shape()[2], 3, "Image must be RGB");
    assert!(array.len() > 0);
    let dims = [array.shape()[0], array.shape()[1]];
    let rgb: Vec<u8> = array
        .data()
        .iter()
        .map(|value| (value.clamp(0., 1.) * 255.0) as u8)
        .collect();
    ImageData::Color(ColorImage::from_rgb(dims, &rgb))
}
