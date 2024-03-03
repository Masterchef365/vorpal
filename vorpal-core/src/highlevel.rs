use std::{collections::HashMap, rc::Rc};

use crate::{ComponentFn, ComponentInfixOp, DataType, ExternInputId, HashRcByPtr, Node, Value};

/// A higher-level, nicer set of nodes. Compiles to the lower-level set ...
/// HighNode is a strict superset of Node. Is always directly convertible to Node.
#[derive(Clone, Debug)]
pub enum HighNode {
    // Components of Node
    ExternInput(ExternInputId, DataType),
    Constant(Value),
    Make(Vec<Rc<HighNode>>, DataType),
    ComponentInfixOp(Rc<HighNode>, ComponentInfixOp, Rc<HighNode>),
    ComponentFn(ComponentFn, Rc<HighNode>),
    GetComponent(Rc<HighNode>, Rc<HighNode>),
    Dot(Rc<HighNode>, Rc<HighNode>),

    // New stuff!
    Normalize(Rc<HighNode>, DataType),
    Splat(Rc<HighNode>, DataType),
    /// Convert one datatype to another and/or rearrange components
    Swizzle {
        /// Input data
        input_vector: Rc<HighNode>,
        /// Input indices
        component_vector: Rc<HighNode>,
        /// Input vector datatype
        input_vector_dtype: DataType,
        /// Matches component vector dtype
        output_vector_dtype: DataType,
    },
}

/// This preserves the identity of each individual node (so its tree will not be copied!)
type Cache = HashMap<HashRcByPtr<HighNode>, Rc<Node>>;

pub fn convert_node(high: Rc<HighNode>) -> Rc<Node> {
    convert_rc_highnode(high, &mut HashMap::new())
}

fn convert_rc_highnode(high: Rc<HighNode>, cache: &mut Cache) -> Rc<Node> {
    if let Some(cached) = cache.get(&HashRcByPtr(high.clone())) {
        // Use the exact same pointer, so that the wasm assembler code using HashRcByPtr later down the line
        // can just assume that the Rc<> has a refcount equal to the number of times it's used in
        // the code instead of re-computing everything each time ...
        //
        // ... I should just use slotmap instead lol
        cached.clone()
    } else {
        lower_node_recursive(high, cache)
    }
}

fn lower_node_recursive(high: Rc<HighNode>, cache: &mut Cache) -> Rc<Node> {
    match Rc::unwrap_or_clone(high) {
        HighNode::ExternInput(id, dtype) => Rc::new(Node::ExternInput(id, dtype)),
        HighNode::Constant(value) => Rc::new(Node::Constant(value)),
        HighNode::Make(components, dtype) => Rc::new(Node::Make(
            components
                .into_iter()
                .map(|c| convert_rc_highnode(c, cache))
                .collect(),
            dtype,
        )),
        HighNode::ComponentInfixOp(left, op, right) => Rc::new(Node::ComponentInfixOp(
            convert_rc_highnode(left, cache),
            op,
            convert_rc_highnode(right, cache),
        )),
        HighNode::ComponentFn(op, data) => {
            Rc::new(Node::ComponentFn(op, convert_rc_highnode(data, cache)))
        }
        HighNode::GetComponent(left, right) => Rc::new(Node::GetComponent(
            convert_rc_highnode(left, cache),
            convert_rc_highnode(right, cache),
        )),
        HighNode::Dot(left, right) => Rc::new(Node::Dot(
            convert_rc_highnode(left, cache),
            convert_rc_highnode(right, cache),
        )),
        // Now here's the more useful stuff
        HighNode::Splat(scalar, dtype) => {
            let scalar = convert_rc_highnode(scalar, cache);
            let copies = (0..dtype.n_lanes()).map(|_| scalar.clone()).collect();
            Rc::new(Node::Make(copies, dtype))
        }
        HighNode::Normalize(vect, dtype) => {
            let len2 = Rc::new(HighNode::Dot(vect.clone(), vect.clone()));

            let half = Rc::new(HighNode::Constant(Value::Scalar(0.5)));
            let len = Rc::new(HighNode::ComponentInfixOp(
                len2,
                ComponentInfixOp::Power,
                half,
            ));
            let len = Rc::new(HighNode::Splat(len, dtype));

            let normed = Rc::new(HighNode::ComponentInfixOp(
                vect.clone(),
                ComponentInfixOp::Divide,
                len,
            ));

            convert_rc_highnode(normed, cache)
        }
        HighNode::Swizzle {
            input_vector,
            component_vector,
            input_vector_dtype,
            output_vector_dtype,
        } => convert_rc_highnode(
            Rc::new(HighNode::Make(
                (0..output_vector_dtype.n_lanes()).map(|lane_idx| {
                    Rc::new(HighNode::GetComponent(
                        input_vector.clone(),
                        Rc::new(HighNode::GetComponent(
                            component_vector.clone(),
                            Rc::new(HighNode::Constant(Value::Scalar(lane_idx as f32))),
                        )),
                    ))
                })
                .collect(),
                output_vector_dtype,
            )),
            cache,
        ),
    }
}
