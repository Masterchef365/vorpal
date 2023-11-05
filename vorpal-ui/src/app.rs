use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};

use eframe::egui::{self, TextEdit};
use ndarray::*;
use vorpal_core::{ndarray, ExternInputId, ExternParameters, Value};

use vorpal_ui::wasmtime_integration::{NodeGraphs, VorpalWasmtime};
use vorpal_widgets::{
    image_view::{array_to_imagedata, ImageViewWidget},
    node_editor::NodeGraphWidget,
};

type FuncName = String;

// ========= First, define your user data types =============
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SaveState {
    user_wasm_path: Option<PathBuf>,
    functions: Vec<(FuncName, NodeGraphWidget)>,
    selected_function: usize,
}

pub struct VorpalApp {
    //use_wasm: bool,
    saved: SaveState,
    image: ImageViewWidget,

    image_data: NdArray<f32>,

    time: Instant,

    autosave_timer: Instant,
    engine: Option<VorpalWasmtime>,
}

const AUTOSAVE_INTERVAL_SECS: f32 = 30.0;

fn default_inputs() -> ExternParameters {
    let inputs = [
        (
            ExternInputId::new(vorpal_ui::TIME_KEY.to_string()),
            Value::Scalar(0.1),
        ),
        (
            ExternInputId::new(vorpal_ui::POS_KEY.to_string()),
            Value::Vec2([0.; 2]),
        ),
        (
            ExternInputId::new(vorpal_ui::RESOLUTION_KEY.to_string()),
            Value::Vec2([1.; 2]),
        ),
        (
            ExternInputId::new(vorpal_ui::CURSOR_KEY.to_string()),
            Value::Vec2([-1.; 2]),
        ),
    ]
    .into_iter()
    .collect();
    ExternParameters {
        inputs,
        samplers: Default::default(),
    }
}

impl Default for SaveState {
    fn default() -> Self {
        let nodes = NodeGraphWidget::new(default_inputs());
        Self {
            user_wasm_path: Some("target/wasm32-unknown-unknown/release/vorpal_image.wasm".into()),
            functions: [("kernel".to_string(), nodes)].into_iter().collect(),
            selected_function: 0,
        }
    }
}

impl Default for VorpalApp {
    fn default() -> Self {
        Self {
            saved: Default::default(),
            engine: None,
            time: Instant::now(),
            autosave_timer: Instant::now(),
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
        let state: SaveState = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, PERSISTENCE_KEY))
            .unwrap_or_default();

        let mut inst = Self::default();

        inst.saved = state;

        inst
    }
}

impl eframe::App for VorpalApp {
    #[cfg(feature = "persistence")]
    /// If the persistence function is enabled,
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PERSISTENCE_KEY, &self.saved);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Load wasm file if unloaded
        if self.engine.is_none() {
            if let Some(path) = &self.saved.user_wasm_path {
                match VorpalWasmtime::new(path.clone()) {
                    Ok(engine) => self.engine = Some(engine),
                    Err(e) => {
                        eprintln!("Failed to load wasmtime {:?}", e);
                        self.saved.user_wasm_path = None;
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

        self.saved.selected_fn_widget().context_mut().insert_input(
            &ExternInputId::new(vorpal_ui::RESOLUTION_KEY.into()),
            Value::Vec2([width as f32, height as f32]),
        );

        // Paint image using native backend
        //if let Ok(Some(node)) = self.saved.nodes.extract_active_node() {
        let nodes: NodeGraphs = self
            .saved
            .functions
            .iter_mut()
            .map(|(name, widget)| (name.clone(), widget.extract_output_node()))
            .collect();

        if let Some(engine) = self.engine.as_mut() {
            match engine.eval_image(&nodes, self.saved.selected_fn_widget().context()) {
                Ok(image_data) => {
                    self.image_data.data_mut().copy_from_slice(&image_data);
                }
                Err(e) => {
                    eprintln!("Error failed to eval {:#}", e);
                    self.image_data
                        .data_mut()
                        .iter_mut()
                        .zip([1., 0., 0., 0.].into_iter().cycle())
                        .for_each(|(o, i)| *o = i);
                }
            }
        }

        /*
        for i in 0..width {
            for j in 0..height {
                self.saved.selected_fn_widget().context_mut().insert_input(
                    &ExternInputId::new(vorpal_ui::POS_KEY.into()),
                    Value::Vec2([i as f32, j as f32]),
                );

                let Ok(Value::Vec4(result)) = evaluate_node(&node, self.saved.selected_fn_widget().context())
                else {
                    panic!("Failed to eval node");
                };

                for (k, component) in result.into_iter().enumerate() {
                    self.image_data[[j, i, k]] = component;
                }
            }
        }
        */

        self.image
            .set_image("my image".into(), ctx, array_to_imagedata(&self.image_data));

        self.saved.selected_fn_widget().context_mut().insert_input(
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

                let filename_text = match self.saved.user_wasm_path.as_ref() {
                    Some(text) => text.display().to_string(), //text.to_str().unwrap().to_string(),
                    None => "No WASM file loaded.".to_string(),
                };
                ui.with_layout(egui::Layout::right_to_left(eframe::emath::Align::Max), |ui| {
                    ui.label(format!("Running {filename_text}"));
                })
                //ui.menu_button(filename_text, |_| ());
            });
        });
        egui::SidePanel::left("nodes").show(ctx, |ui| {
            self.saved.selected_fn_widget().show(ui);
        });
        egui::SidePanel::right("options").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Function name editor
                let mut remove: Option<usize> = None;

                for (idx, (function_name, _)) in self.saved.functions.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Edit function name
                        ui.add(TextEdit::singleline(function_name).desired_width(100.));

                        // Deletion
                        if ui.button("Delete").clicked() {
                            remove = Some(idx);
                        }

                        // Selection
                        if ui
                            .selectable_label(self.saved.selected_function == idx, "Select")
                            .clicked()
                        {
                            self.saved.selected_function = idx;
                        }
                    });
                }
                if ui.button("New").clicked() {
                    self.saved
                        .functions
                        .push(("unnamed".into(), NodeGraphWidget::new(default_inputs())));
                }

                if let Some(idx) = remove {
                    self.saved.functions.remove(idx);
                }
            })
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let response = self.image.show(ui);
            if response.clicked() | response.dragged() {
                let cursor_pos = response
                    .interact_pointer_pos()
                    .unwrap_or(egui::Pos2::new(-1., -1.));
                let rel_pos = cursor_pos - response.rect.min;
                let image_shape = self.image_data.shape();
                let image_size_vect = egui::Vec2::new(image_shape[0] as f32, image_shape[1] as f32);
                let pixel_pos = image_size_vect * rel_pos / response.rect.size();

                self.saved.selected_fn_widget().context_mut().insert_input(
                    &ExternInputId::new(vorpal_ui::CURSOR_KEY.into()),
                    Value::Vec2(pixel_pos.into()),
                );
            }
        });
    }
}

impl VorpalApp {
    pub fn load_user_wasm_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Load .wasm file")
            .pick_file()
        {
            self.saved.user_wasm_path = Some(path);
            // Require reloading the engine
            self.engine = None;
        }
    }

    pub fn save_wat_file(&self) {
        if let Some(engine) = self.engine.as_ref() {
            if let Some(cache) = engine.cache.as_ref() {
                let (func_name, _) = &self.saved.functions[self.saved.selected_function];
                if let Ok(wat) =
                    cache.analyses[self.saved.selected_function].compile_to_wat(&func_name)
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save .wat file")
                        .set_file_name(format!("{}.wat", func_name))
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
            if let Err(e) = self.saved.save_vor_file(&path) {
                eprintln!("Error saving {}; {:?}", path.display(), e);
            }
        }
    }

    pub fn load_vor_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open .vor file")
            .pick_file()
        {
            self.saved = SaveState::load_vor_file(path).unwrap();
        }
    }
}

impl SaveState {
    pub fn save_vor_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    pub fn load_vor_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }
}

impl SaveState {
    pub fn selected_fn_widget(&mut self) -> &mut NodeGraphWidget {
        if self.functions.is_empty() {
            self.functions.push((
                "unnamed".to_string(),
                NodeGraphWidget::new(default_inputs()),
            ));
        }

        self.selected_function = self.selected_function.min(self.functions.len() - 1);

        &mut self.functions[self.selected_function].1
    }
}
