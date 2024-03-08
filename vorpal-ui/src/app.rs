use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::format_err;

use eframe::{
    egui::{self, ComboBox, Label, Layout, RichText, ScrollArea, TextEdit, Ui},
    epaint::Color32,
};
use ndarray::*;
use vorpal_core::{ndarray, DataType, ExternInputId, ExternParameters, ParameterList, Value, Vec2, highlevel};

use vorpal_ui::wasmtime_integration::{NodeGraphs, VorpalWasmtime};
use vorpal_widgets::{
    image_view::{array_to_imagedata, ImageViewWidget},
    node_editor::NodeGraphWidget,
};

type FuncName = String;

// ========= First, define your user data types =============
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[serde(default)]
pub struct SaveState {
    user_wasm_path: Option<PathBuf>,
    functions: Vec<(FuncName, NodeGraphWidget)>,
    selected_function: usize,
    show_wat: bool,
    show_rust_decl: bool,
    pause: bool,
    focused: bool,
}

pub struct VorpalApp {
    //use_wasm: bool,
    saved: SaveState,
    image: ImageViewWidget,

    image_data: NdArray<f32>,

    time: Instant,

    autosave_timer: Instant,
    engine: Option<VorpalWasmtime>,
    single_step: bool,

    /// Cursor pos relative to the image size (in units of image's pixels)
    cursor_pos: Option<Vec2>,
    add_dtype: DataType,
    add_param: String,
}

const AUTOSAVE_INTERVAL_SECS: f32 = 30.0;

fn image_fn_inputs() -> ParameterList {
    let params = [
        (
            ExternInputId::new(vorpal_ui::TIME_KEY.to_string()),
            DataType::Scalar,
        ),
        (
            ExternInputId::new(vorpal_ui::POS_KEY.to_string()),
            DataType::Vec2,
        ),
        (
            ExternInputId::new(vorpal_ui::RESOLUTION_KEY.to_string()),
            DataType::Vec2,
        ),
        (
            ExternInputId::new(vorpal_ui::CURSOR_KEY.to_string()),
            DataType::Vec2,
        ),
    ]
    .into_iter()
    .collect();

    ParameterList(params)
}

impl Default for SaveState {
    fn default() -> Self {
        let nodes = new_widget();
        Self {
            user_wasm_path: Some("target/wasm32-unknown-unknown/release/vorpal_image.wasm".into()),
            functions: [("kernel".to_string(), nodes)].into_iter().collect(),
            selected_function: 0,
            show_wat: false,
            pause: false,
            focused: false,
            show_rust_decl: true,
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
            image_data: NdArray::zeros(vec![200, 200, 4]),
            // Start with a single step, in order to show the initial texture...
            single_step: true,
            cursor_pos: None,
            add_dtype: DataType::Scalar,
            add_param: "my_new_param".into(),
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
        if !self.saved.pause || self.single_step {
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

            let extern_parameters = [
                (
                    ExternInputId::new(vorpal_ui::RESOLUTION_KEY.into()),
                    Value::Vec2([width as f32, height as f32]),
                ),
                (
                    ExternInputId::new(vorpal_ui::CURSOR_KEY.into()),
                    Value::Vec2(self.cursor_pos.unwrap_or([-1., -1.]).into()),
                ),
                (
                    ExternInputId::new(vorpal_ui::TIME_KEY.to_string()),
                    Value::Scalar(self.time.elapsed().as_secs_f32()),
                ),
            ];
            let extern_parameters = ExternParameters::new(extern_parameters.into_iter().collect());

            // Paint image using native backend
            //if let Ok(Some(node)) = self.saved.nodes.extract_active_node() {
            let nodes: NodeGraphs = self
                .saved
                .functions
                .iter_mut()
                .map(|(name, widget)| {
                    (
                        name.clone(),
                        highlevel::convert_node(widget.extract_output_node()),
                        widget.params().clone(),
                    )
                })
                .collect();

            if let Some(engine) = self.engine.as_mut() {
                match engine.eval_image(&nodes, &extern_parameters) {
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

            self.image
                .set_image("my image".into(), ctx, array_to_imagedata(&self.image_data));

            self.single_step = false;
        }

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
                //ui.menu_button("Control", |ui| {
                if ui.button("Single step").clicked() {
                    self.saved.pause = true;
                    self.single_step = true;
                }
                ui.checkbox(&mut self.saved.pause, "Pause");
                if ui.button("Reset").clicked() {
                    self.engine = None;
                }
                ui.checkbox(&mut self.saved.focused, "Focused");
                //});

                let filename_text = match self.saved.user_wasm_path.as_ref() {
                    Some(path) => path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .map(|s| s.to_string())
                        .unwrap(),
                    None => "No WASM file loaded.".to_string(),
                };

                ui.with_layout(
                    egui::Layout::right_to_left(eframe::emath::Align::Max),
                    |ui| {
                        ui.label(format!("Running {filename_text}"));
                    },
                );
                //ui.menu_button(filename_text, |_| ());
            });
        });

        if !self.saved.focused {
            egui::SidePanel::left("nodes").show(ctx, |ui| {
                self.saved.selected_fn_widget().show(ui);
            });
            egui::SidePanel::right("options").show(ctx, |ui| {
                ui.strong("Functions");

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
                    self.saved.selected_function = self.saved.functions.len();

                    self.saved
                        .functions
                        .push(("unnamed".into(), new_widget()));
                }

                if let Some(idx) = remove {
                    self.saved.functions.remove(idx);
                }

                ui.separator();

                ui.strong("Selected function parameters");

                let param_list = self.saved.selected_fn_widget().params_mut();

                let mut ordered: Vec<ExternInputId> = param_list.0.iter().map(|(id, _)| id.clone()).collect();
                ordered.sort();

                let mut delete = None;
                for (idx, id) in ordered.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(id.to_string());
                        let selected = param_list.0.iter_mut().find_map(|(p_id, dtype)| (p_id == id).then(|| dtype)).unwrap();
                        dtype_selector(idx, ui, selected);
                        if ui.button("delete").clicked() {
                            delete = Some(idx);
                        }
                    });
                }

                if let Some(idx) = delete {
                    let to_remove = &ordered[idx];
                    param_list.0.retain(|(id, _)| id != to_remove);
                }

                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        param_list
                            .0
                            .push((ExternInputId::new(self.add_param.clone()), self.add_dtype));
                    }
                    ui.text_edit_singleline(&mut self.add_param);
                    dtype_selector(99999, ui, &mut self.add_dtype)
                });

                // Get function name
                let func_name = &self.saved.functions[self.saved.selected_function].0;

                // Get rust function body as a string
                let maybe_fn_body: Option<String> = self.engine.as_ref().and_then(|engine| {
                    engine
                        .cache
                        .as_ref()?
                        .analyses
                        .get(self.saved.selected_function)?
                        .func_name_rust(&func_name)
                        .ok()
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.saved.show_rust_decl, "Rust function declaration:");
                    if ui.button("Copy").clicked() {
                        if let Some(function_body) = &maybe_fn_body {
                            ui.output_mut(|o| o.copied_text = function_body.clone());
                        }
                    }
                });
                if self.saved.show_rust_decl {
                    // Display that function body
                    if let Some(mut function_body) = maybe_fn_body {
                        ScrollArea::horizontal()
                            .id_source("for rust function body")
                            .show(ui, |ui| {
                                // Not actually editing text here!!
                                ui.add(
                                    TextEdit::multiline(&mut function_body)
                                        .code_editor()
                                        .desired_width(f32::INFINITY),
                                );
                            });
                    }
                }

                ui.separator();

                ui.checkbox(&mut self.saved.show_wat, "WebAssembly text:");
                if self.saved.show_wat {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Show wasm code
                        let mut text = self
                            .engine
                            .as_ref()
                            .ok_or(format_err!("No engine"))
                            .and_then(|engine| {
                                engine.cache.as_ref().ok_or(format_err!("Nothing cached"))
                            })
                            .and_then(|cache| {
                                cache
                                    .analyses
                                    .get(self.saved.selected_function)
                                    .ok_or(format_err!("Nothing selected"))
                            })
                            .and_then(|analysis| {
                                analysis
                                    .compile_to_wat(&func_name)
                                    .map_err(|e| format_err!("Compilation failed {:?}", e))
                            })
                            .unwrap_or_else(|err| err.to_string());
                        ui.add(
                            TextEdit::multiline(&mut text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            /*
            let response = ui
                .with_layout(
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| self.image.show(ui),
                )
                .inner;
            */
            let response = self.image.show(ui);

            self.cursor_pos = (response.clicked() || response.dragged()).then(|| {
                let cursor_pos = response
                    .interact_pointer_pos()
                    .unwrap_or(egui::Pos2::new(-1., -1.));
                let rel_pos = cursor_pos - response.rect.min;
                let image_shape = self.image_data.shape();
                let image_size_vect = egui::Vec2::new(image_shape[0] as f32, image_shape[1] as f32);
                let pixel_pos = image_size_vect * rel_pos / response.rect.size();

                pixel_pos.into()
            });
        });
    }
}

impl VorpalApp {
    pub fn load_user_wasm_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Load .wasm file")
            .add_filter("wasm", &["wasm"])
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
            .add_filter("vor", &["vor"])
            .pick_file()
        {
            self.saved = SaveState::load_vor_file(path).unwrap();
            self.engine = None;
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

fn new_widget() -> NodeGraphWidget {
    NodeGraphWidget::new(image_fn_inputs(), DataType::Vec4, "RGBA".into())
}

impl SaveState {
    pub fn selected_fn_widget(&mut self) -> &mut NodeGraphWidget {
        if self.functions.is_empty() {
            self.functions.push((
                "unnamed".to_string(),
                new_widget()
            ));
        }

        self.selected_function = self.selected_function.min(self.functions.len() - 1);

        &mut self.functions[self.selected_function].1
    }
}

fn dtype_selector(idx: usize, ui: &mut Ui, dtype: &mut DataType) {
    ComboBox::new((idx, "dtype selector"), "")
        .selected_text(dtype.to_string())
        .show_ui(ui, |ui| {
            for alt_dtype in DataType::all() {
                ui.selectable_value(dtype, alt_dtype, alt_dtype.to_string());
            }
        });
}
