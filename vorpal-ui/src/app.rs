use std::{path::PathBuf, time::Instant};

use eframe::egui::{self, ScrollArea, TextStyle};
use ndarray::*;
use vorpal_core::{native_backend::evaluate_node, ndarray, ExternInputId, Value};

use vorpal_ui::wasmtime_integration::VorpalWasmtime;
use vorpal_widgets::{
    image_view::{array_to_imagedata, ImageViewWidget},
    node_editor::NodeGraphWidget,
};

// ========= First, define your user data types =============

pub struct VorpalApp {
    nodes: NodeGraphWidget,
    image: ImageViewWidget,

    user_wasm_path: Option<PathBuf>,

    image_data: NdArray<f32>,

    time: Instant,

    autosave_timer: Instant,
    use_wasm: bool,
    engine: Option<VorpalWasmtime>,
}

const AUTOSAVE_INTERVAL_SECS: f32 = 30.0;

impl Default for VorpalApp {
    fn default() -> Self {
        let mut nodes = NodeGraphWidget::default();
        nodes.context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::TIME_KEY.to_string()),
            Value::Scalar(0.1),
        );
        nodes.context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::POS_KEY.to_string()),
            Value::Vec2([0.; 2]),
        );
        nodes.context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::RESOLUTION_KEY.to_string()),
            Value::Vec2([1.; 2]),
        );

        Self {
            user_wasm_path: Some("target/wasm32-unknown-unknown/release/vorpal_image.wasm".into()),
            engine: None,
            use_wasm: true,
            time: Instant::now(),
            autosave_timer: Instant::now(),
            nodes,
            image: Default::default(),
            image_data: NdArray::zeros(vec![100, 100, 4]),
        }
    }
}

#[cfg(feature = "persistence")]
const PERSISTENCE_KEY: &str = "egui_node_graph";

#[cfg(feature = "persistence")]
impl VorpalApp {
    /// If the persistence feature is enabled, Called once before the first frame.
    /// Load previous app state (if any).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let state: NodeGraphWidget = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, PERSISTENCE_KEY))
            .unwrap_or_default();

        let mut inst = Self::default();

        inst.nodes = state;

        inst
        //Self::default()
    }
}

impl eframe::App for VorpalApp {
    #[cfg(feature = "persistence")]
    /// If the persistence function is enabled,
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PERSISTENCE_KEY, &self.nodes);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Load wasm file if unloaded
        if self.engine.is_none() {
            if let Some(path) = &self.user_wasm_path {
                match VorpalWasmtime::new(path.clone()) {
                    Ok(engine) => self.engine = Some(engine),
                    Err(e) => {
                        eprintln!("{:?}", e);
                        self.user_wasm_path = None;
                    }
                }
            }
        }

        // Autosave
        if self.autosave_timer.elapsed().as_secs_f32() > AUTOSAVE_INTERVAL_SECS {
            self.autosave_timer = Instant::now();

            if let Some(storage) = frame.storage_mut() {
                self.save(storage);
                storage.flush();
                eprintln!("Autosave successful");
            }
        }

        ctx.request_repaint();

        let width = self.image_data.shape()[0];
        let height = self.image_data.shape()[1];

        self.nodes.context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::RESOLUTION_KEY.into()),
            Value::Vec2([width as f32, height as f32]),
        );

        // Paint image using native backend
        //if let Ok(Some(node)) = self.nodes.extract_active_node() {
        let node = self.nodes.extract_output_node();
        if self.use_wasm {
            if let Some(engine) = self.engine.as_mut() {
                match engine.eval_image(&node, self.nodes.context()) {
                    Ok(image_data) => {
                        self.image_data.data_mut().copy_from_slice(&image_data);
                    }
                    Err(e) => {
                        eprintln!("Error {:#}", e);
                        self.image_data
                            .data_mut()
                            .iter_mut()
                            .zip([1., 0., 0., 0.].into_iter().cycle())
                            .for_each(|(o, i)| *o = i);
                    }
                }
            }
        } else {
            for i in 0..width {
                for j in 0..height {
                    self.nodes.context_mut().insert_input(
                        &ExternInputId::new(vorpal_ui::POS_KEY.into()),
                        Value::Vec2([i as f32, j as f32]),
                    );

                    let Ok(Value::Vec4(result)) = evaluate_node(&node, self.nodes.context()) else {
                        panic!("Failed to eval node");
                    };

                    for (k, component) in result.into_iter().enumerate() {
                        self.image_data[[j, i, k]] = component;
                    }
                }
            }
        }

        self.image
            .set_image("my image".into(), ctx, array_to_imagedata(&self.image_data));

        self.nodes.context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::TIME_KEY.to_string()),
            Value::Scalar(self.time.elapsed().as_secs_f32()),
        );

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.menu_button("File", |ui| {
                    if ui.button("Save .wat (compiled wasm module text)").clicked() {
                        self.save_wat_file();
                    }
                    if ui.button("Save .vor (nodes)").clicked() {
                        self.save_vor_file();
                    }
                    if ui.button("Load .vor (nodes)").clicked() {
                        self.load_vor_file();
                    }
                    if ui.button("Load .wasm (user code)").clicked() {
                        self.load_user_wasm_file();
                    }
                    if ui.button("Load defaults").clicked() {
                        *self = Self::default();
                    }
                });

                let filename_text = match self.user_wasm_path.as_ref() {
                    Some(text) => text.to_str().unwrap().to_string(),
                    None => "No WASM file loaded.".to_string(),
                };
                ui.label(filename_text);
                //ui.menu_button(filename_text, |_| ());
            });
        });
        egui::SidePanel::left("nodes").show(ctx, |ui| {
            self.nodes.show(ui);
        });
        egui::SidePanel::right("options").show(ctx, |ui| {
            ui.checkbox(&mut self.use_wasm, "Use WASM");
            let maybe_node = self.nodes.extract_active_node();

            let text = match maybe_node {
                Ok(Some(node)) => {
                    /*
                    let result = match self.use_wasm {
                        true => self.engine.eval(&node, self.nodes.context()),
                        false => {
                    */
                    let result =
                        vorpal_core::native_backend::evaluate_node(&node, self.nodes.context());
                    /*
                                .map_err(|e| e.into())
                        }
                    };
                    */

                    match result {
                        Err(e) => format!("Error: {:?}", e),
                        Ok(value) => format!("The result is: {:?}", value),
                    }
                }
                Ok(None) => format!("No node selected"),
                Err(err) => format!("Execution error: {}", err),
            };

            if let Some(engine) = self.engine.as_ref() {
                if let Some(cache) = engine.cache.as_ref() {
                    ScrollArea::vertical().show(ui, |ui| {
                        let text = cache.anal.compile_to_wat().unwrap();
                        ui.label(&text);
                    });
                }
            }

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

impl VorpalApp {
    pub fn load_user_wasm_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Load .wasm file")
            .pick_file()
        {
            self.user_wasm_path = Some(path);
            // Require reloading the engine
            self.engine = None;
        }
    }

    pub fn save_wat_file(&self) {
        if let Some(engine) = self.engine.as_ref() {
            if let Some(cache) = engine.cache.as_ref() {
                if let Ok(wat) = cache.anal.compile_to_wat() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save .wat file")
                        .set_file_name("project.wat")
                        .save_file()
                    {
                        if let Err(e) = std::fs::write(path, &wat) {
                            eprintln!("Error saving .wat: {:#}", e)
                        }
                    }
                }
            }
        }
    }

    pub fn save_vor_file(&self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save .vor file")
            .set_file_name("project.vor")
            .save_file()
        {
            if let Ok(file) = std::fs::File::create(path) {
                let _ = serde_json::to_writer_pretty(file, self.nodes.state());
            }
        }
    }

    pub fn load_vor_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open .vor file")
            .pick_file()
        {
            let file = std::fs::File::open(path).unwrap();
            let state = serde_json::from_reader(file).unwrap();
            self.nodes.set_state(state);
        }
    }
}
