use std::time::Instant;

use eframe::{
    egui::{self, TextStyle, TextureOptions, Ui},
    epaint::{ColorImage, ImageData, ImageDelta, TextureId, Vec2},
};
use ndarray::*;
use vorpal_core::{native_backend::evaluate_node, ndarray, ExternInputId, Value};

use crate::node_editor::*;

// ========= First, define your user data types =============

pub struct NodeGraphExample {
    nodes: NodeGraphWidget,
    image: ImageViewWidget,
    image_data: NdArray<f32>,
    time: Instant,

    use_wasm: bool,
    engine: vorpal_wasm::Engine,
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
            engine: vorpal_wasm::Engine::new().unwrap(),
            use_wasm: true,
            time: Instant::now(),
            nodes,
            image: Default::default(),
            image_data: NdArray::zeros(vec![100, 100, 4]),
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

impl NodeGraphExample {
    fn save_wat_file(&self) {}
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

        let width = self.image_data.shape()[0];
        let height = self.image_data.shape()[1];

        self.nodes.context_mut().insert_input(
            &ExternInputId::new(RESOLUTION_KEY.into()),
            Value::Vec2([width as f32, height as f32]),
        );

        // Paint image using native backend
        //if let Ok(Some(node)) = self.nodes.extract_active_node() {
        let node = self.nodes.extract_output_node();
        if self.use_wasm {
            let image_data = self.engine.eval_image(&node, &self.nodes.context).unwrap();
            self.image_data.data_mut().copy_from_slice(&image_data);
        } else {
            for i in 0..width {
                for j in 0..height {
                    self.nodes.context_mut().insert_input(
                        &ExternInputId::new(POS_KEY.into()),
                        Value::Vec2([i as f32, j as f32]),
                    );

                    let Ok(Value::Vec4(result)) = evaluate_node(&node, &self.nodes.context) else {
                        panic!("Failed to eval node");
                    };

                    for (k, component) in result.into_iter().enumerate() {
                        self.image_data[[i, j, k]] = component;
                    }
                }
            }
        }

        self.image
            .set_image("my image".into(), ctx, array_to_imagedata(&self.image_data));

        self.nodes.context_mut().insert_input(
            &ExternInputId::new(TIME_KEY.to_string()),
            Value::Scalar(self.time.elapsed().as_secs_f32()),
        );

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.menu_button("File", |ui| {
                    if ui.button("Save .wat").clicked() {
                        self.save_wat_file();
                    }
                });
            });
        });
        egui::SidePanel::left("nodes").show(ctx, |ui| {
            self.nodes.show(ui);
        });
        egui::SidePanel::right("options").show(ctx, |ui| {
            ui.checkbox(&mut self.use_wasm, "Use WASM for active node");
            let maybe_node = self.nodes.extract_active_node();

            let text = match maybe_node {
                Ok(Some(node)) => {
                    let result = match self.use_wasm {
                        true => self.engine.eval(&node, &self.nodes.context),
                        false => {
                            vorpal_core::native_backend::evaluate_node(&node, &self.nodes.context)
                                .map_err(|e| e.into())
                        }
                    };

                    match result {
                        Err(e) => format!("Error: {:?}", e),
                        Ok(value) => format!("The result is: {:?}", value),
                    }
                }
                Ok(None) => format!("No node selected"),
                Err(err) => format!("Execution error: {}", err),
            };

            ui.ctx().debug_painter().text(
                egui::pos2(10.0, 35.0),
                egui::Align2::LEFT_TOP,
                text,
                TextStyle::Button.resolve(&ui.ctx().style()),
                egui::Color32::WHITE,
            );
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
    assert_eq!(array.shape()[2], 4, "Image must be RGBA");
    assert!(array.len() > 0);
    let dims = [array.shape()[0], array.shape()[1]];
    let mut rgba: Vec<u8> = array
        .data()
        .iter()
        .map(|value| (value.clamp(0., 1.) * 255.0) as u8)
        .collect();

    // Set alpha to one. TODO: UNDO THIS!!
    rgba.iter_mut().skip(3).step_by(4).for_each(|v| *v = u8::MAX);

    ImageData::Color(ColorImage::from_rgba_unmultiplied(dims, &rgba))
}
