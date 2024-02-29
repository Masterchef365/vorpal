use std::{rc::Rc, collections::HashMap};

use crate::{ComponentFn, ComponentInfixOp, DataType, ExternInputId, Node, Value, HashRcByPtr};

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
    Normalize(Rc<HighNode>),
    Swizzle(Swizzle, Rc<HighNode>),
}

#[derive(Clone, Debug)]
pub enum Swizzle {}

/// This preserves the identity of each individual node (so its tree will not be copied!)
type Cache = HashMap<HashRcByPtr<HighNode>, Rc<Node>>;

fn convert_rc_highnode(high: Rc<HighNode>, cache: &mut Cache) -> Rc<Node> {
    if let Some(cached) = cache.get(&HashRcByPtr(high.clone())) {
        // Use the exact same pointer, so that the wasm assembler code using HashRcByPtr later down the line
        // can just assume that the Rc<> has a refcount equal to the number of times it's used in
        // the code instead of re-computing everything each time ...
        //
        // ... I should just use slotmap instead lol
        cached.clone()
    } else {
        lower_node(high, cache)
    }
}

fn lower_node(high: Rc<HighNode>, cache: &mut Cache) -> Rc<Node> {
    Rc::new(match Rc::unwrap_or_clone(high) {
        HighNode::ExternInput(id, dtype) => Node::ExternInput(id, dtype),
        HighNode::Constant(value) => Node::Constant(value),
        HighNode::Make(components, dtype) => Node::Make(
            components.into_iter().map(|c| convert_rc_highnode(c, cache)).collect(),
            dtype,
        ),
        HighNode::ComponentInfixOp(left, op, right) => {
            Node::ComponentInfixOp(convert_rc_highnode(left, cache), op, convert_rc_highnode(right, cache))
        }
        HighNode::ComponentFn(op, data) => Node::ComponentFn(op, convert_rc_highnode(data, cache)),
        HighNode::GetComponent(left, right) => {
            Node::GetComponent(convert_rc_highnode(left, cache), convert_rc_highnode(right, cache))
        }
        HighNode::Dot(left, right) => {
            Node::Dot(convert_rc_highnode(left, cache), convert_rc_highnode(right, cache))
        }
        /*
        // Now here's the more useful stuff
        HighNode::Normalize(vect) => {
            let vect = convert_rc_highnode(vect);
            let v2 = Rc::new(Node::Dot(vect.clone(), vect.clone()));

        }
        */
        _ => todo!(),
    })
}

impl From<HighNode> for Node {
    fn from(value: HighNode) -> Self {
        Rc::unwrap_or_clone(lower_node(Rc::new(value), &mut HashMap::new()))
    }
}
