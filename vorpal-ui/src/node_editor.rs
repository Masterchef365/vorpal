use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    rc::Rc,
};

const XYZW: [&str; 4] = ["x", "y", "z", "w"];

use eframe::egui::{self, ComboBox, DragValue, TextStyle, Ui};
use egui_node_graph::*;
use vorpal_core::*;

pub struct NodeGraphWidget {
    // The `GraphEditorState` is the top-level object. You "register" all your
    // custom types by specifying it as its generic parameters.
    context: ExternContext,
    state: MyEditorState,
    user_state: MyGraphState,
}

pub type MyGraph = Graph<MyNodeData, DataType, NodeGuiValue>;
pub type MyEditorState =
    GraphEditorState<MyNodeData, DataType, NodeGuiValue, MyNodeTemplate, MyGraphState>;

/// The NodeData holds a custom data struct inside each node. It's useful to
/// store additional information that doesn't live in parameters. For this
/// example, the node data stores the template (i.e. the "type") of the node.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MyNodeData {
    pub template: MyNodeTemplate,
}

#[derive(Copy, Clone, Debug)]
pub struct NodeGuiValue(pub Value);

/// NodeTemplate is a mechanism to define node templates. It's what the graph
/// will display in the "new node" popup. The user code needs to tell the
/// library how to convert a NodeTemplate into a Node.
#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MyNodeTemplate {
    Input(ExternInputId, DataType),
    Make(DataType),
    ComponentInfixOp(ComponentInfixOp, DataType),
    ComponentFn(ComponentFn, DataType),
    GetComponent(DataType),
    Output(DataType),
    Dot(DataType),
}

/// The response type is used to encode side-effects produced when drawing a
/// node in the graph. Most side-effects (creating new nodes, deleting existing
/// nodes, handling connections...) are already handled by the library, but this
/// mechanism allows creating additional side effects from user code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyResponse {
    SetActiveNode(NodeId),
    ClearActiveNode,
    SetComponentInfixOp(NodeId, ComponentInfixOp),
    SetComponentFn(NodeId, ComponentFn),
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
            DataType::Vec3 => egui::Color32::LIGHT_GREEN,
            DataType::Vec4 => egui::Color32::DARK_BLUE,
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            DataType::Scalar => Cow::Borrowed("scalar"),
            DataType::Vec2 => Cow::Borrowed("2d vector"),
            DataType::Vec3 => Cow::Borrowed("3d vector"),
            DataType::Vec4 => Cow::Borrowed("4d vector"),
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
        Cow::Owned(match self {
            Self::Make(dtype) => format!("Make {dtype}"),
            Self::ComponentInfixOp(_infix, dtype) => format!("Operator ({dtype})"),
            Self::ComponentFn(_func, dtype) => format!("Function ({dtype})"),
            Self::GetComponent(dtype) => format!("Get component ({dtype})"),
            Self::Input(name, dtype) => format!("Input {name} ({dtype})"),
            Self::Output(dtype) => format!("Output ({dtype})"),
            Self::Dot(dtype) => format!("Dot ({dtype})"),
        })
    }

    // this is what allows the library to show collapsible lists in the node finder.
    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<&'static str> {
        match self {
            MyNodeTemplate::Make(dtype)
            | MyNodeTemplate::ComponentInfixOp(_, dtype)
            | MyNodeTemplate::ComponentFn(_, dtype)
            | MyNodeTemplate::GetComponent(dtype)
            | MyNodeTemplate::Dot(dtype) => vec![dtype.dtype_name()],
            MyNodeTemplate::Input(_name, dtype) => vec!["Input", dtype.dtype_name()],
            MyNodeTemplate::Output(_) => vec![],
        }
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        // It's okay to delegate this to node_finder_label if you don't want to
        // show different names in the node finder and the node itself.
        self.node_finder_label(user_state).into()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        MyNodeData {
            template: self.clone(),
        }
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
        let add_input = |graph: &mut MyGraph, name: &str, dtype: DataType| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                dtype,
                NodeGuiValue(Value::default_of_dtype(dtype)),
                InputParamKind::ConnectionOrConstant,
                true,
            )
        };

        let add_output = |graph: &mut MyGraph, name: &str, dtype: DataType| {
            graph.add_output_param(node_id, name.to_string(), dtype);
        };

        match self {
            MyNodeTemplate::Make(dtype) => {
                for part in XYZW.iter().take(dtype.lanes()) {
                    add_input(graph, *part, DataType::Scalar);
                }
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::ComponentFn(_func, dtype) => {
                add_input(graph, "x", *dtype);
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::GetComponent(dtype) => {
                add_input(graph, "value", *dtype);
                add_input(graph, "index", DataType::Scalar);
                add_output(graph, "out", DataType::Scalar);
            }
            MyNodeTemplate::ComponentInfixOp(_comp, dtype) => {
                add_input(graph, "x", *dtype);
                add_input(graph, "y", *dtype);
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::Input(_name, dtype) => {
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::Output(dtype) => {
                add_input(graph, "x", *dtype);
            }
            MyNodeTemplate::Dot(dtype) => {
                add_input(graph, "x", *dtype);
                add_input(graph, "y", *dtype);
                add_output(graph, "out", DataType::Scalar);
            }
        }
    }
}

pub struct AllMyNodeTemplates<'ctx> {
    ctx: &'ctx ExternContext,
}

impl NodeTemplateIter for AllMyNodeTemplates<'_> {
    type Item = MyNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        // This function must return a list of node kinds, which the node finder
        // will use to display it to the user. Crates like strum can reduce the
        // boilerplate in enumerating all variants of an enum.
        let mut types = vec![];
        for dtype in DataType::all() {
            types.push(MyNodeTemplate::Make(dtype));
            types.push(MyNodeTemplate::ComponentInfixOp(
                ComponentInfixOp::Add,
                dtype,
            ));
            types.push(MyNodeTemplate::GetComponent(dtype));
            types.push(MyNodeTemplate::ComponentFn(ComponentFn::NaturalLog, dtype));
            types.push(MyNodeTemplate::Dot(dtype));
        }

        for (id, value) in self.ctx.inputs() {
            types.push(MyNodeTemplate::Input(id.clone(), value.dtype()));
        }

        types
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

        ui.label(param_name);

        let mut input_vector = |vector: &mut [f32]| {
            ui.horizontal(|ui| {
                for (num, name) in vector.iter_mut().zip(XYZW) {
                    ui.label(name);
                    ui.add(DragValue::new(num));
                }
            });
        };

        match self {
            Self(Value::Vec2(value)) => input_vector(value),
            Self(Value::Vec3(value)) => input_vector(value),
            Self(Value::Vec4(value)) => input_vector(value),
            Self(Value::Scalar(value)) => {
                ui.horizontal(|ui| {
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

        match self.template {
            MyNodeTemplate::ComponentFn(mut func, _dtype) => {
                let mut updated = false;
                ComboBox::new(node_id, "Function")
                    .selected_text(func.to_string())
                    .show_ui(ui, |ui| {
                        for val in ComponentFn::all() {
                            updated |= ui
                                .selectable_value(&mut func, val, val.to_string())
                                .clicked();
                        }
                    });
                if updated {
                    responses.push(NodeResponse::User(MyResponse::SetComponentFn(
                        node_id, func,
                    )));
                }
            }
            MyNodeTemplate::ComponentInfixOp(mut infix, _dtype) => {
                let mut updated = false;
                ComboBox::new(node_id, "Operation")
                    .selected_text(infix.to_string())
                    .show_ui(ui, |ui| {
                        for val in ComponentInfixOp::all() {
                            updated |= ui
                                .selectable_value(&mut infix, val, val.to_string())
                                .clicked();
                        }
                    });

                if updated {
                    responses.push(NodeResponse::User(MyResponse::SetComponentInfixOp(
                        node_id, infix,
                    )));
                }
            }
            _ => (),
        }

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

/// Detects whether there is a cycle in determining the output of the given node
pub fn detect_cycle(graph: &MyGraph, node_id: NodeId) -> bool {
    detect_cycle_recursive(graph, node_id, &mut HashSet::new())
}

fn detect_cycle_recursive(graph: &MyGraph, node_id: NodeId, stack: &mut HashSet<NodeId>) -> bool {
    // If we encounter node_id twice in one depth-first sweep of the graph, then we have a cycle!
    if !stack.insert(node_id) {
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

pub fn extract_node(graph: &MyGraph, node_id: NodeId) -> anyhow::Result<Rc<vorpal_core::Node>> {
    extract_node_recursive(graph, node_id, &mut OutputsCache::new())
}

// Returns the ID of the vorpal_core::Node corresponding to given parameter of the node "node_id"
fn extract_node_recursive(
    graph: &MyGraph,
    node_id: NodeId,
    cache: &mut OutputsCache,
) -> anyhow::Result<Rc<vorpal_core::Node>> {
    let node = &graph[node_id];

    if node.outputs(graph).next().is_some() {
        let output_id = node.get_output("out")?;
        if let Some(cached) = cache.get(&output_id) {
            return Ok(cached.clone());
        }
    }

    Ok(match &node.user_data.template {
        MyNodeTemplate::ComponentFn(func, _dtype) => Rc::new(vorpal_core::Node::ComponentFn(
            *func,
            get_input_node(graph, node_id, "x", cache)?,
        )),
        MyNodeTemplate::GetComponent(_dtype) => Rc::new(vorpal_core::Node::GetComponent(
            get_input_node(graph, node_id, "value", cache)?,
            get_input_node(graph, node_id, "index", cache)?,
        )),
        MyNodeTemplate::ComponentInfixOp(op, _dtype) => {
            Rc::new(vorpal_core::Node::ComponentInfixOp(
                get_input_node(graph, node_id, "x", cache)?,
                *op,
                get_input_node(graph, node_id, "y", cache)?,
            ))
        }
        MyNodeTemplate::Make(dtype) => Rc::new(vorpal_core::Node::Make(
            XYZW.iter()
                .take(dtype.lanes())
                .map(|name| get_input_node(graph, node_id, name, cache))
                .collect::<Result<_, _>>()?,
            *dtype,
        )),
        MyNodeTemplate::Input(name, _dtype) => {
            Rc::new(vorpal_core::Node::ExternInput(name.clone()))
        }
        MyNodeTemplate::Output(_dtype) => get_input_node(graph, node_id, "x", cache)?,
        MyNodeTemplate::Dot(_dtype) => {
            Rc::new(vorpal_core::Node::Dot(
                get_input_node(graph, node_id, "x", cache)?,
                get_input_node(graph, node_id, "y", cache)?,
            ))
        }
    })
}

type OutputsCache = HashMap<OutputId, Rc<vorpal_core::Node>>;

/// Recursively evaluates all dependencies of this node, then evaluates the node itself.
pub fn evaluate_graph_node(
    graph: &MyGraph,
    node_id: NodeId,
    context: &ExternContext,
) -> anyhow::Result<NodeGuiValue> {
    Ok(NodeGuiValue(vorpal_core::evaluate_node(
        &*extract_node(graph, node_id)?,
        context,
    )?))
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

impl NodeGraphWidget {
    pub fn context_mut(&mut self) -> &mut ExternContext {
        &mut self.context
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let before: HashSet<InputId> = self.state.graph.connections.keys().collect();
        let resp = self.state.draw_graph_editor(
            ui,
            AllMyNodeTemplates { ctx: &self.context },
            &mut self.user_state,
            Vec::default(),
        );
        let after: HashSet<InputId> = self.state.graph.connections.keys().collect();

        if let Some(added) = after.difference(&before).next() {
            undo_if_cycle(*added, &mut self.state.graph);
        }

        for node_response in resp.node_responses {
            // Here, we ignore all other graph events. But you may find
            // some use for them. For example, by playing a sound when a new
            // connection is created
            if let NodeResponse::User(user_event) = node_response {
                match user_event {
                    MyResponse::SetActiveNode(node) => self.user_state.active_node = Some(node),
                    MyResponse::ClearActiveNode => self.user_state.active_node = None,
                    MyResponse::SetComponentInfixOp(id, infix) => {
                        match &mut self.state.graph[id].user_data.template {
                            MyNodeTemplate::ComponentInfixOp(current_infix, _) => {
                                *current_infix = infix
                            }
                            _ => panic!("Wrong message"),
                        }
                    }
                    MyResponse::SetComponentFn(id, func) => {
                        match &mut self.state.graph[id].user_data.template {
                            MyNodeTemplate::ComponentFn(current_func, _) => *current_func = func,
                            _ => panic!("Wrong message"),
                        }
                    }
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
                    match evaluate_graph_node(&self.state.graph, node, &self.context) {
                        Ok(NodeGuiValue(value)) => {
                            format!("The result is: {:?}\n{:#?}", value, extracted)
                        }
                        Err(err) => format!("Execution error: {}", err),
                    }
                };

                ui.ctx().debug_painter().text(
                    egui::pos2(10.0, 35.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    TextStyle::Button.resolve(&ui.ctx().style()),
                    egui::Color32::WHITE,
                );
            } else {
                self.user_state.active_node = None;
            }
        }
    }

    pub fn eval_output_node(&self) -> anyhow::Result<Value> {
        let node_id = self.state.graph.nodes.iter().find_map(|(id, node)| {
            matches!(node.user_data.template, MyNodeTemplate::Output(_)).then(|| id)
        }).unwrap();
        evaluate_graph_node(&self.state.graph, node_id, &self.context).map(|NodeGuiValue(val)| val)
    }
}

fn undo_if_cycle(input_id: InputId, graph: &mut MyGraph) {
    let node_id = graph.get_input(input_id).node;
    if detect_cycle(graph, node_id) {
        graph.remove_connection(input_id);
    }
}

impl Default for NodeGraphWidget {
    fn default() -> Self {
        let mut state: MyEditorState = MyEditorState::default();
        let mut user_state: MyGraphState = Default::default();

        let output = MyNodeTemplate::Output(DataType::Vec4);

        let template = output.clone();
        let id = state
            .graph
            .add_node("Output".into(), MyNodeData { template }, |_, _| ());

        state.node_positions.insert(id, egui::Pos2::ZERO);
        state.node_order.push(id);
        MyNodeTemplate::Output(DataType::Vec4).build_node(&mut state.graph, &mut user_state, id);

        Self {
            context: Default::default(),
            state,
            user_state,
        }
    }
}
