use egui::{self, Color32, ComboBox, DragValue, Ui};
use egui_node_graph::*;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    rc::Rc,
};
use vorpal_core::highlevel::HighNode;
use vorpal_core::*;

const XYZW: [&str; 4] = ["x", "y", "z", "w"];

/// Widget allowing the user to interactively design
/// a function using a node and connection paradigm
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct NodeGraphWidget {
    params: ParameterList,
    state: MyEditorState,
    user_state: MyGraphState,
}

type MyGraph = Graph<MyNodeData, DataType, NodeGuiValue>;
pub type MyEditorState =
    GraphEditorState<MyNodeData, DataType, NodeGuiValue, MyNodeTemplate, MyGraphState>;

/// The NodeData holds a custom data struct inside each node. It's useful to
/// store additional information that doesn't live in parameters. For this
/// example, the node data stores the template (i.e. the "type") of the node.
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct MyNodeData {
    template: MyNodeTemplate,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug)]
pub struct NodeGuiValue(Value);

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
    Normalize(DataType),
    Splat(DataType),
    Swizzle(DataType, DataType),
    Comment,
}

/// The response type is used to encode side-effects produced when drawing a
/// node in the graph. Most side-effects (creating new nodes, deleting existing
/// nodes, handling connections...) are already handled by the library, but this
/// mechanism allows creating additional side effects from user code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MyResponse {
    SetActiveNode(NodeId),
    ClearActiveNode,
    SetComponentInfixOp(NodeId, ComponentInfixOp),
    SetComponentFn(NodeId, ComponentFn),
    SetComment(NodeId, String),
}

/// The graph 'global' state. This state struct is passed around to the node and
/// parameter drawing callbacks. The contents of this struct are entirely up to
/// the user. For this example, we use it to keep track of the 'active' node.
#[derive(Clone)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MyGraphState {
    active_node: Option<NodeId>,
    comments: UniqueSecondaryMap<NodeId, String>,
}

// =========== Then, you need to implement some traits ============

// A trait for the data types, to tell the library how to display them
impl DataTypeTrait<MyGraphState> for DataType {
    fn data_type_color(&self, _user_state: &mut MyGraphState) -> egui::Color32 {
        match self {
            DataType::Scalar => Color32::from_rgb(22, 99, 169),
            DataType::Vec2 => Color32::from_rgb(149, 0, 0),
            DataType::Vec3 => Color32::from_rgb(33, 121, 18),
            DataType::Vec4 => Color32::from_rgb(78, 19, 133),
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
            Self::Splat(dtype) => format!("Splat {dtype}"),
            Self::Normalize(dtype) => format!("Normalize {dtype}"),
            Self::Make(dtype) => format!("Make {dtype}"),
            Self::ComponentInfixOp(_infix, dtype) => format!("Math Operator ({dtype})"),
            Self::ComponentFn(_func, dtype) => format!("Math Function ({dtype})"),
            Self::GetComponent(dtype) => format!("Get component ({dtype})"),
            Self::Input(name, dtype) => format!("Input {name} ({dtype})"),
            Self::Output(dtype) => format!("Output ({dtype})"),
            Self::Dot(dtype) => format!("Dot ({dtype})"),
            Self::Swizzle(dtype, other_dtype) => format!("Swizzle {dtype} -> {other_dtype}"),
            Self::Comment => format!("Comment"),
        })
    }

    // this is what allows the library to show collapsible lists in the node finder.
    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<&'static str> {
        match self {
            MyNodeTemplate::Make(dtype)
            | MyNodeTemplate::Splat(dtype)
            | MyNodeTemplate::ComponentInfixOp(_, dtype)
            | MyNodeTemplate::ComponentFn(_, dtype)
            | MyNodeTemplate::GetComponent(dtype)
            | MyNodeTemplate::Normalize(dtype)
            | MyNodeTemplate::Dot(dtype) => vec![dtype.dtype_name()],
            MyNodeTemplate::Input(_name, dtype) => vec!["Input", dtype.dtype_name()],
            MyNodeTemplate::Swizzle(dtype, _other_dtype) => {
                //vec![dtype.dtype_name(), other_dtype.dtype_name()]
                match dtype {
                    // Don't bother showing the swizzles that result in a scalar;
                    // these are already GetComponent
                    DataType::Scalar => vec![],
                    DataType::Vec2 => vec!["Swizzle Vec2"],
                    DataType::Vec3 => vec!["Swizzle Vec3"],
                    DataType::Vec4 => vec!["Swizzle Vec4"],
                }
            }
            MyNodeTemplate::Output(_) => vec![],
            MyNodeTemplate::Comment => vec!["Util"],
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
        user_state: &mut Self::UserState,
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
                for part in XYZW.iter().take(dtype.n_lanes()) {
                    add_input(graph, *part, DataType::Scalar);
                }
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::Splat(dtype) => {
                add_input(graph, "x", DataType::Scalar);
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
            MyNodeTemplate::Normalize(dtype) => {
                add_input(graph, "x", *dtype);
                add_output(graph, "out", *dtype);
            }
            MyNodeTemplate::Swizzle(input_dtype, output_dtype) => {
                add_input(graph, "x", *input_dtype);
                add_input(graph, "indices", *output_dtype);
                add_output(graph, "out", *output_dtype);
            }
            MyNodeTemplate::Comment => {}
        }
    }
}

struct AllMyNodeTemplates<'ctx> {
    params: &'ctx ParameterList,
}

impl NodeTemplateIter for AllMyNodeTemplates<'_> {
    type Item = MyNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        // This function must return a list of node kinds, which the node finder
        // will use to display it to the user. Crates like strum can reduce the
        // boilerplate in enumerating all variants of an enum.
        let mut types = vec![];
        for dtype in DataType::all() {
            // Redundant
            if dtype != DataType::Scalar {
                types.push(MyNodeTemplate::Splat(dtype));
            }
            types.push(MyNodeTemplate::Normalize(dtype));
            types.push(MyNodeTemplate::Make(dtype));
            types.push(MyNodeTemplate::ComponentInfixOp(
                ComponentInfixOp::Add,
                dtype,
            ));
            if dtype != DataType::Scalar {
                // Redundant
                types.push(MyNodeTemplate::GetComponent(dtype));

                // Handled by splat
                for output_dtype in DataType::all() {
                    if output_dtype != DataType::Scalar {
                        types.push(MyNodeTemplate::Swizzle(dtype, output_dtype));
                    }
                }
            }
            types.push(MyNodeTemplate::ComponentFn(ComponentFn::NaturalLog, dtype));
            types.push(MyNodeTemplate::Dot(dtype));
        }

        for (id, dtype) in self.params.inputs() {
            types.push(MyNodeTemplate::Input(id.clone(), *dtype));
        }

        types.push(MyNodeTemplate::Comment);

        types
    }
}

pub fn input_scalar(x: &mut Scalar) -> DragValue {
    DragValue::new(x).speed(1e-2)
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

        let input_vector = |ui: &mut Ui, vector: &mut [f32]| {
            ui.horizontal(|ui| {
                for (num, name) in vector.iter_mut().zip(XYZW) {
                    ui.label(name);
                    ui.add(input_scalar(num));
                }
            });
        };

        match self {
            Self(Value::Vec2(value)) => input_vector(ui, value),
            Self(Value::Vec3(value)) => {
                ui.horizontal(|ui| {
                    input_vector(ui, value);
                    srgb_edit(ui, value);
                });
            }
            Self(Value::Vec4(value)) => {
                input_vector(ui, value);
                premultiplied_srgba_edit(ui, value);
            }
            Self(Value::Scalar(value)) => {
                ui.horizontal(|ui| {
                    ui.add(input_scalar(value));
                });
            }
        }
        // This allows you to return your responses from the inline widgets.
        Vec::new()
    }
}

fn srgb_edit(ui: &mut Ui, value: &mut [f32; 3]) {
    let mut srgb = value.map(|v| (v.clamp(0., 1.) * 256.) as u8);
    if ui.color_edit_button_srgb(&mut srgb).changed() {
        *value = srgb.map(|v| v as f32 / 256.);
    }
}

fn premultiplied_srgba_edit(ui: &mut Ui, value: &mut [f32; 4]) {
    let [r, g, b, a] = value.map(|v| (v.clamp(0., 1.) * 256.) as u8);
    let mut srgba: Color32 = Color32::from_rgba_premultiplied(r, g, b, a);
    if ui.color_edit_button_srgba(&mut srgba).changed() {
        *value = srgba.to_array().map(|v| v as f32 / 256.);
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
        graph: &Graph<MyNodeData, DataType, NodeGuiValue>,
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
                    .width(ui.style().spacing.slider_width)
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
                    .width(ui.style().spacing.slider_width)
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
            MyNodeTemplate::Comment => {
                let mut s = if user_state.comments.contains_key(node_id) {
                    user_state.comments[node_id].clone()
                } else {
                    String::new()
                };
                if ui.text_edit_multiline(&mut s).changed() {
                    responses.push(NodeResponse::User(MyResponse::SetComment(node_id, s)));
                }
            }
            _ => (),
        }

        let is_active = user_state
            .active_node
            .map(|id| id == node_id)
            .unwrap_or(false);

        /*
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
        */

        responses
    }

    fn titlebar_color(
        &self,
        _ui: &egui::Ui,
        node_id: NodeId,
        graph: &Graph<Self, Self::DataType, Self::ValueType>,
        user_state: &mut Self::UserState,
    ) -> Option<egui::Color32> {
        graph[node_id]
            .user_data
            .template
            .get_datatype()
            .map(|dtype| dtype.data_type_color(user_state))
    }

    fn can_delete(
        &self,
        node_id: NodeId,
        graph: &Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> bool {
        !matches!(graph[node_id].user_data.template, MyNodeTemplate::Output(_))
    }
}

/// Detects whether there is a cycle in determining the output of the given node
fn detect_cycle(graph: &MyGraph, node_id: NodeId) -> bool {
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

fn extract_node_from_graph(graph: &MyGraph, node_id: NodeId) -> anyhow::Result<Rc<HighNode>> {
    extract_node_from_graph_recursive(graph, node_id, &mut OutputsCache::new())
}

// Returns the ID of the HighNode corresponding to given parameter of the node "node_id"
fn extract_node_from_graph_recursive(
    graph: &MyGraph,
    node_id: NodeId,
    cache: &mut OutputsCache,
) -> anyhow::Result<Rc<HighNode>> {
    let node = &graph[node_id];

    if node.outputs(graph).next().is_some() {
        let output_id = node.get_output("out")?;
        if let Some(cached) = cache.get(&output_id) {
            return Ok(cached.clone());
        }
    }

    Ok(match &node.user_data.template {
        MyNodeTemplate::ComponentFn(func, _dtype) => Rc::new(HighNode::ComponentFn(
            *func,
            get_input_node(graph, node_id, "x", cache)?,
        )),
        MyNodeTemplate::GetComponent(_dtype) => Rc::new(HighNode::GetComponent(
            get_input_node(graph, node_id, "value", cache)?,
            get_input_node(graph, node_id, "index", cache)?,
        )),
        MyNodeTemplate::ComponentInfixOp(op, _dtype) => Rc::new(HighNode::ComponentInfixOp(
            get_input_node(graph, node_id, "x", cache)?,
            *op,
            get_input_node(graph, node_id, "y", cache)?,
        )),
        MyNodeTemplate::Make(dtype) => Rc::new(HighNode::Make(
            XYZW.iter()
                .take(dtype.n_lanes())
                .map(|name| get_input_node(graph, node_id, name, cache))
                .collect::<Result<_, _>>()?,
            *dtype,
        )),
        MyNodeTemplate::Input(name, dtype) => Rc::new(HighNode::ExternInput(name.clone(), *dtype)),
        MyNodeTemplate::Output(_dtype) => get_input_node(graph, node_id, "x", cache)?,
        MyNodeTemplate::Dot(_dtype) => Rc::new(HighNode::Dot(
            get_input_node(graph, node_id, "x", cache)?,
            get_input_node(graph, node_id, "y", cache)?,
        )),
        MyNodeTemplate::Normalize(dtype) => Rc::new(HighNode::Normalize(
            get_input_node(graph, node_id, "x", cache)?,
            *dtype,
        )),
        MyNodeTemplate::Splat(dtype) => Rc::new(HighNode::Splat(
            get_input_node(graph, node_id, "x", cache)?,
            *dtype,
        )),
        MyNodeTemplate::Swizzle(input_dtype, output_dtype) => Rc::new(HighNode::Swizzle {
            input_vector: get_input_node(graph, node_id, "x", cache)?,
            component_vector: get_input_node(graph, node_id, "indices", cache)?,
            input_vector_dtype: *input_dtype,
            output_vector_dtype: *output_dtype,
        }),
        MyNodeTemplate::Comment => unreachable!(),
    })
}

type OutputsCache = HashMap<OutputId, Rc<HighNode>>;

fn get_input_node(
    graph: &MyGraph,
    node_id: NodeId,
    param_name: &str,
    cache: &mut OutputsCache,
) -> anyhow::Result<Rc<HighNode>> {
    let input_id = graph[node_id].get_input(param_name)?;

    // The output of another node is connected.
    if let Some(other_output_id) = graph.connection(input_id) {
        let node = extract_node_from_graph_recursive(graph, graph[other_output_id].node, cache)?;
        cache.insert(other_output_id, node.clone());
        Ok(node)
    }
    // No existing connection, take the inline value instead.
    else {
        let NodeGuiValue(value) = graph[input_id].value;
        Ok(Rc::new(HighNode::Constant(value)))
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
    /// Create a new nodegraph widget with the given input list
    pub fn new(params: ParameterList, output_dtype: DataType, output_name: String) -> Self {
        let mut state: MyEditorState = MyEditorState::default();
        let mut user_state: MyGraphState = MyGraphState {
            active_node: None,
            comments: UniqueSecondaryMap::new_from_key(&state.graph.nodes),
        };

        let output = MyNodeTemplate::Output(output_dtype);

        let template = output.clone();
        let id = state
            .graph
            .add_node(output_name, MyNodeData { template }, |_, _| ());

        state.node_positions.insert(id, egui::Pos2::ZERO);
        state.node_order.push(id);
        MyNodeTemplate::Output(output_dtype).build_node(&mut state.graph, &mut user_state, id);

        Self {
            params,
            state,
            user_state,
        }
    }

    pub fn state(&self) -> &MyEditorState {
        &self.state
    }

    pub fn set_state(&mut self, state: MyEditorState) {
        self.state = state;
    }

    pub fn params(&self) -> &ParameterList {
        &self.params
    }

    pub fn params_mut(&mut self) -> &mut ParameterList {
        &mut self.params
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let before: HashSet<InputId> = self.state.graph.connections.keys().collect();
        let resp = self.state.draw_graph_editor(
            ui,
            AllMyNodeTemplates {
                params: &self.params,
            },
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
                    MyResponse::SetComment(id, text) => {
                        self.user_state.comments.insert(id, text);
                    }
                }
            }
        }
    }

    pub fn extract_active_node(&mut self) -> anyhow::Result<Option<Rc<HighNode>>> {
        if let Some(node) = self.user_state.active_node {
            if self.state.graph.nodes.contains_key(node) {
                if detect_cycle(&self.state.graph, node) {
                    Err(anyhow::format_err!("Cycle detected"))
                } else {
                    let extracted = extract_node_from_graph(&self.state.graph, node).unwrap();
                    Ok(Some(extracted))
                }
            } else {
                self.user_state.active_node = None;
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn extract_output_node(&mut self) -> Rc<HighNode> {
        let node_id = self
            .state
            .graph
            .nodes
            .iter()
            .find_map(|(id, node)| {
                matches!(node.user_data.template, MyNodeTemplate::Output(_)).then(|| id)
            })
            .unwrap();
        let extracted = extract_node_from_graph(&self.state.graph, node_id).unwrap();
        extracted
    }
}

fn undo_if_cycle(input_id: InputId, graph: &mut MyGraph) {
    let node_id = graph.get_input(input_id).node;
    if detect_cycle(graph, node_id) {
        graph.remove_connection(input_id);
    }
}

impl MyNodeTemplate {
    /*
    fn set_datatype(&mut self, input: DataType) {
        match self {
            MyNodeTemplate::Input(_, dtype)
            | MyNodeTemplate::Make(dtype)
            | MyNodeTemplate::ComponentInfixOp(_, dtype)
            | MyNodeTemplate::ComponentFn(_, dtype)
            | MyNodeTemplate::GetComponent(dtype)
            | MyNodeTemplate::Output(dtype)
            | MyNodeTemplate::Normalize(dtype)
            | MyNodeTemplate::Dot(dtype) => *dtype = input,
        }
    }
    */

    fn get_datatype(&self) -> Option<DataType> {
        match self {
            MyNodeTemplate::Input(_, dtype)
            | MyNodeTemplate::Splat(dtype)
            | MyNodeTemplate::Make(dtype)
            | MyNodeTemplate::ComponentInfixOp(_, dtype)
            | MyNodeTemplate::ComponentFn(_, dtype)
            | MyNodeTemplate::GetComponent(dtype)
            | MyNodeTemplate::Output(dtype)
            | MyNodeTemplate::Normalize(dtype)
            | MyNodeTemplate::Swizzle(_, dtype)
            | MyNodeTemplate::Dot(dtype) => Some(*dtype),
            MyNodeTemplate::Comment => None,
        }
    }
}
