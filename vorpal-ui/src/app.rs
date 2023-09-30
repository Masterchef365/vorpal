use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use eframe::egui::{self, DragValue, TextStyle};
use egui_node_graph::*;
use vorpal_core::*;

// ========= First, define your user data types =============

/// The NodeData holds a custom data struct inside each node. It's useful to
/// store additional information that doesn't live in parameters. For this
/// example, the node data stores the template (i.e. the "type") of the node.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MyNodeData {
    template: MyNodeTemplate,
}

#[derive(Copy, Clone, Debug)]
pub struct NodeGuiValue(pub Value);

impl NodeGuiValue {
    /// Tries to downcast this value type to a vector
    fn try_to_vec2(self) -> anyhow::Result<egui::Vec2> {
        if let Self(Value::Vec2(value)) = self {
            Ok(value.into())
        } else {
            anyhow::bail!("Invalid cast from {:?} to vec2", self)
        }
    }

    /// Tries to downcast this value type to a scalar
    fn try_to_scalar(self) -> anyhow::Result<f32> {
        if let Self(Value::Scalar(value)) = self {
            Ok(value)
        } else {
            anyhow::bail!("Invalid cast from {:?} to scalar", self)
        }
    }
}

/// NodeTemplate is a mechanism to define node templates. It's what the graph
/// will display in the "new node" popup. The user code needs to tell the
/// library how to convert a NodeTemplate into a Node.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MyNodeTemplate {
    MakeScalar,
    AddScalar,
    SubtractScalar,
    MakeVector,
    AddVector,
    SubtractVector,
    VectorTimesScalar,
}

/// The response type is used to encode side-effects produced when drawing a
/// node in the graph. Most side-effects (creating new nodes, deleting existing
/// nodes, handling connections...) are already handled by the library, but this
/// mechanism allows creating additional side effects from user code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyResponse {
    SetActiveNode(NodeId),
    ClearActiveNode,
}

/// The graph 'global' state. This state struct is passed around to the node and
/// parameter drawing callbacks. The contents of this struct are entirely up to
/// the user. For this example, we use it to keep track of the 'active' node.
#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MyGraphState {
    pub active_node: Option<NodeId>,
}

// =========== Then, you need to implement some traits ============

// A trait for the data types, to tell the library how to display them
impl DataTypeTrait<MyGraphState> for DataType {
    fn data_type_color(&self, _user_state: &mut MyGraphState) -> egui::Color32 {
        match self {
            DataType::Scalar => egui::Color32::from_rgb(38, 109, 211),
            DataType::Vec2 => egui::Color32::from_rgb(238, 207, 109),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            DataType::Scalar => Cow::Borrowed("scalar"),
            DataType::Vec2 => Cow::Borrowed("2d vector"),
        }
    }
}

// A trait for the node kinds, which tells the library how to build new nodes
// from the templates in the node finder
impl NodeTemplateTrait for MyNodeTemplate {
    type NodeData = MyNodeData;
    type DataType = DataType;
    type ValueType = NodeGuiValue;
    type UserState = MyGraphState;
    type CategoryType = &'static str;

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        Cow::Borrowed(match self {
            MyNodeTemplate::MakeScalar => "New scalar",
            MyNodeTemplate::AddScalar => "Scalar add",
            MyNodeTemplate::SubtractScalar => "Scalar subtract",
            MyNodeTemplate::MakeVector => "New vector",
            MyNodeTemplate::AddVector => "Vector add",
            MyNodeTemplate::SubtractVector => "Vector subtract",
            MyNodeTemplate::VectorTimesScalar => "Vector times scalar",
        })
    }

    // this is what allows the library to show collapsible lists in the node finder.
    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<&'static str> {
        match self {
            MyNodeTemplate::MakeScalar
            | MyNodeTemplate::AddScalar
            | MyNodeTemplate::SubtractScalar => vec!["Scalar"],
            MyNodeTemplate::MakeVector
            | MyNodeTemplate::AddVector
            | MyNodeTemplate::SubtractVector => vec!["Vector"],
            MyNodeTemplate::VectorTimesScalar => vec!["Vector", "Scalar"],
        }
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        // It's okay to delegate this to node_finder_label if you don't want to
        // show different names in the node finder and the node itself.
        self.node_finder_label(user_state).into()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        MyNodeData { template: *self }
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
        node_id: NodeId,
    ) {
        // The nodes are created empty by default. This function needs to take
        // care of creating the desired inputs and outputs based on the template

        // We define some closures here to avoid boilerplate. Note that this is
        // entirely optional.
        let input_scalar = |graph: &mut MyGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                DataType::Scalar,
                NodeGuiValue(Value::Scalar(0.0)),
                InputParamKind::ConnectionOrConstant,
                true,
            );
        };
        let input_vector = |graph: &mut MyGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                DataType::Vec2,
                NodeGuiValue(Value::Vec2([0.0, 0.0])),
                InputParamKind::ConnectionOrConstant,
                true,
            );
        };

        let output_scalar = |graph: &mut MyGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), DataType::Scalar);
        };
        let output_vector = |graph: &mut MyGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), DataType::Vec2);
        };

        match self {
            MyNodeTemplate::AddScalar => {
                // The first input param doesn't use the closure so we can comment
                // it in more detail.
                graph.add_input_param(
                    node_id,
                    // This is the name of the parameter. Can be later used to
                    // retrieve the value. Parameter names should be unique.
                    "A".into(),
                    // The data type for this input. In this case, a scalar
                    DataType::Scalar,
                    // The value type for this input. We store zero as default
                    NodeGuiValue(Value::Scalar(0.0)),
                    // The input parameter kind. This allows defining whether a
                    // parameter accepts input connections and/or an inline
                    // widget to set its value.
                    InputParamKind::ConnectionOrConstant,
                    true,
                );
                input_scalar(graph, "B");
                output_scalar(graph, "out");
            }
            MyNodeTemplate::SubtractScalar => {
                input_scalar(graph, "A");
                input_scalar(graph, "B");
                output_scalar(graph, "out");
            }
            MyNodeTemplate::VectorTimesScalar => {
                input_scalar(graph, "scalar");
                input_vector(graph, "vector");
                output_vector(graph, "out");
            }
            MyNodeTemplate::AddVector => {
                input_vector(graph, "v1");
                input_vector(graph, "v2");
                output_vector(graph, "out");
            }
            MyNodeTemplate::SubtractVector => {
                input_vector(graph, "v1");
                input_vector(graph, "v2");
                output_vector(graph, "out");
            }
            MyNodeTemplate::MakeVector => {
                input_scalar(graph, "x");
                input_scalar(graph, "y");
                output_vector(graph, "out");
            }
            MyNodeTemplate::MakeScalar => {
                input_scalar(graph, "value");
                output_scalar(graph, "out");
            }
        }
    }
}

pub struct AllMyNodeTemplates;
impl NodeTemplateIter for AllMyNodeTemplates {
    type Item = MyNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        // This function must return a list of node kinds, which the node finder
        // will use to display it to the user. Crates like strum can reduce the
        // boilerplate in enumerating all variants of an enum.
        vec![
            MyNodeTemplate::MakeScalar,
            MyNodeTemplate::MakeVector,
            MyNodeTemplate::AddScalar,
            MyNodeTemplate::SubtractScalar,
            MyNodeTemplate::AddVector,
            MyNodeTemplate::SubtractVector,
            MyNodeTemplate::VectorTimesScalar,
        ]
    }
}

impl WidgetValueTrait for NodeGuiValue {
    type Response = MyResponse;
    type UserState = MyGraphState;
    type NodeData = MyNodeData;
    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: NodeId,
        ui: &mut egui::Ui,
        _user_state: &mut MyGraphState,
        _node_data: &MyNodeData,
    ) -> Vec<MyResponse> {
        // This trait is used to tell the library which UI to display for the
        // inline parameter widgets.
        match self {
            Self(Value::Vec2(value)) => {
                ui.label(param_name);
                ui.horizontal(|ui| {
                    ui.label("x");
                    ui.add(DragValue::new(&mut value[0]));
                    ui.label("y");
                    ui.add(DragValue::new(&mut value[1]));
                });
            }
            Self(Value::Scalar(value)) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(DragValue::new(value));
                });
            }
        }
        // This allows you to return your responses from the inline widgets.
        Vec::new()
    }
}

impl UserResponseTrait for MyResponse {}
impl NodeDataTrait for MyNodeData {
    type Response = MyResponse;
    type UserState = MyGraphState;
    type DataType = DataType;
    type ValueType = NodeGuiValue;

    // This method will be called when drawing each node. This allows adding
    // extra ui elements inside the nodes. In this case, we create an "active"
    // button which introduces the concept of having an active node in the
    // graph. This is done entirely from user code with no modifications to the
    // node graph library.
    fn bottom_ui(
        &self,
        ui: &mut egui::Ui,
        node_id: NodeId,
        _graph: &Graph<MyNodeData, DataType, NodeGuiValue>,
        user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<MyResponse, MyNodeData>>
    where
        MyResponse: UserResponseTrait,
    {
        // This logic is entirely up to the user. In this case, we check if the
        // current node we're drawing is the active one, by comparing against
        // the value stored in the global user state, and draw different button
        // UIs based on that.

        let mut responses = vec![];
        let is_active = user_state
            .active_node
            .map(|id| id == node_id)
            .unwrap_or(false);

        // Pressing the button will emit a custom user response to either set,
        // or clear the active node. These responses do nothing by themselves,
        // the library only makes the responses available to you after the graph
        // has been drawn. See below at the update method for an example.
        if !is_active {
            if ui.button("👁 Set active").clicked() {
                responses.push(NodeResponse::User(MyResponse::SetActiveNode(node_id)));
            }
        } else {
            let button =
                egui::Button::new(egui::RichText::new("👁 Active").color(egui::Color32::BLACK))
                    .fill(egui::Color32::GOLD);
            if ui.add(button).clicked() {
                responses.push(NodeResponse::User(MyResponse::ClearActiveNode));
            }
        }

        responses
    }
}

type MyGraph = Graph<MyNodeData, DataType, NodeGuiValue>;
type MyEditorState =
    GraphEditorState<MyNodeData, DataType, NodeGuiValue, MyNodeTemplate, MyGraphState>;

#[derive(Default)]
pub struct NodeGraphExample {
    // The `GraphEditorState` is the top-level object. You "register" all your
    // custom types by specifying it as its generic parameters.
    state: MyEditorState,

    user_state: MyGraphState,
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
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });
        let graph_response = egui::CentralPanel::default()
            .show(ctx, |ui| {
                self.state.draw_graph_editor(
                    ui,
                    AllMyNodeTemplates,
                    &mut self.user_state,
                    Vec::default(),
                )
            })
            .inner;
        for node_response in graph_response.node_responses {
            // Here, we ignore all other graph events. But you may find
            // some use for them. For example, by playing a sound when a new
            // connection is created
            if let NodeResponse::User(user_event) = node_response {
                match user_event {
                    MyResponse::SetActiveNode(node) => self.user_state.active_node = Some(node),
                    MyResponse::ClearActiveNode => self.user_state.active_node = None,
                }
            }
        }

        if let Some(node) = self.user_state.active_node {
            if self.state.graph.nodes.contains_key(node) {
                let has_cycle = detect_cycle(&self.state.graph, node);

                let text = if has_cycle {
                    format!("Cycle detected")
                } else {
                    let extracted = extract_node(&self.state.graph, node).unwrap();
                    match evaluate_node(&self.state.graph, node) {
                        Ok(NodeGuiValue(value)) => {
                            format!("The result is: {:?}\n{:#?}", value, extracted)
                        }
                        Err(err) => format!("Execution error: {}", err),
                    }
                };

                ctx.debug_painter().text(
                    egui::pos2(10.0, 35.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    TextStyle::Button.resolve(&ctx.style()),
                    egui::Color32::WHITE,
                );
            } else {
                self.user_state.active_node = None;
            }
        }
    }
}

type OutputsCache = HashMap<OutputId, Rc<vorpal_core::Node>>;

/// Recursively evaluates all dependencies of this node, then evaluates the node itself.
pub fn evaluate_node(graph: &MyGraph, node_id: NodeId) -> anyhow::Result<NodeGuiValue> {
    Ok(NodeGuiValue(vorpal_core::evaluate_node(&*extract_node(
        graph, node_id,
    )?)?))
}

pub fn extract_node(graph: &MyGraph, node_id: NodeId) -> anyhow::Result<Rc<vorpal_core::Node>> {
    extract_node_recursive(graph, node_id, &mut OutputsCache::new())
}

// Returns the ID of the vorpal_core::Node corresponding to given parameter of the node "node_id"
pub fn extract_node_recursive(
    graph: &MyGraph,
    node_id: NodeId,
    cache: &mut OutputsCache,
) -> anyhow::Result<Rc<vorpal_core::Node>> {
    let node = &graph[node_id];

    let output_id = node.get_output("out")?;

    if let Some(cached) = cache.get(&output_id) {
        return Ok(cached.clone());
    }

    Ok(match node.user_data.template {
        MyNodeTemplate::AddScalar => Rc::new(vorpal_core::Node::AddScalar(
            get_input_node(graph, node_id, "A", cache)?,
            get_input_node(graph, node_id, "B", cache)?,
        )),
        MyNodeTemplate::SubtractScalar => Rc::new(vorpal_core::Node::SubtractScalar(
            get_input_node(graph, node_id, "A", cache)?,
            get_input_node(graph, node_id, "B", cache)?,
        )),
        MyNodeTemplate::VectorTimesScalar => Rc::new(vorpal_core::Node::Vec2TimesScalar(
            get_input_node(graph, node_id, "scalar", cache)?,
            get_input_node(graph, node_id, "vector", cache)?,
        )),
        MyNodeTemplate::AddVector => Rc::new(vorpal_core::Node::AddVec2(
            get_input_node(graph, node_id, "v1", cache)?,
            get_input_node(graph, node_id, "v2", cache)?,
        )),
        MyNodeTemplate::SubtractVector => Rc::new(vorpal_core::Node::SubtractVec2(
            get_input_node(graph, node_id, "v1", cache)?,
            get_input_node(graph, node_id, "v2", cache)?,
        )),
        MyNodeTemplate::MakeVector => Rc::new(vorpal_core::Node::MakeVec2(
            get_input_node(graph, node_id, "x", cache)?,
            get_input_node(graph, node_id, "y", cache)?,
        )),
        MyNodeTemplate::MakeScalar => get_input_node(graph, node_id, "value", cache)?,
    })
}

fn get_input_node(
    graph: &MyGraph,
    node_id: NodeId,
    param_name: &str,
    cache: &mut OutputsCache,
) -> anyhow::Result<Rc<vorpal_core::Node>> {
    let input_id = graph[node_id].get_input(param_name)?;

    // The output of another node is connected.
    if let Some(other_output_id) = graph.connection(input_id) {
        let node = extract_node_recursive(graph, graph[other_output_id].node, cache)?;
        cache.insert(other_output_id, node.clone());
        Ok(node)
    }
    // No existing connection, take the inline value instead.
    else {
        let NodeGuiValue(value) = graph[input_id].value;
        Ok(Rc::new(vorpal_core::Node::Constant(value)))
    }
}

impl Default for NodeGuiValue {
    fn default() -> Self {
        // NOTE: This is just a dummy `Default` implementation. The nodge graph library
        // requires it to circumvent some internal borrow checker issues.
        Self(Value::Scalar(0.0))
    }
}

/// Detects whether there is a cycle in determining the output of the given node
fn detect_cycle(graph: &MyGraph, node_id: NodeId) -> bool {
    detect_cycle_recursive(graph, node_id, &mut HashSet::new())
}

fn detect_cycle_recursive(graph: &MyGraph, node_id: NodeId, stack: &mut HashSet<NodeId>) -> bool {
    // If we encounter node_id twice in one depth-first sweep of the graph, then we have a cycle!
    if !stack.insert(dbg!(node_id)) {
        return true;
    }

    // For each input to this node
    for input_id in graph[node_id].input_ids() {
        // Determine whether the node whoese output this input is connected
        if let Some(other_output_id) = graph.connection(input_id) {
            let other_node_id = graph.outputs[other_output_id].node;
            // Contains a cycle of its own
            if detect_cycle_recursive(graph, other_node_id, stack) {
                return true;
            }
            stack.remove(&other_node_id);
        }
    }

    false
}
