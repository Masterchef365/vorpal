use std::rc::Rc;

use crate::{ComponentFn, ComponentInfixOp, DataType, ExternInputId, Node, Value};

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

fn convert_rc_highnode(high: Rc<HighNode>) -> Rc<Node> {
    Rc::new(Rc::unwrap_or_clone(high).into())
}

impl From<HighNode> for Node {
    fn from(value: HighNode) -> Self {
        match value {
            HighNode::ExternInput(id, dtype) => Node::ExternInput(id, dtype),
            HighNode::Constant(value) => Node::Constant(value),
            HighNode::Make(components, dtype) => Node::Make(
                components.into_iter().map(convert_rc_highnode).collect(),
                dtype,
            ),
            HighNode::ComponentInfixOp(left, op, right) => {
                Node::ComponentInfixOp(convert_rc_highnode(left), op, convert_rc_highnode(right))
            }
            HighNode::ComponentFn(op, data) => Node::ComponentFn(op, convert_rc_highnode(data)),
            HighNode::GetComponent(left, right) => {
                Node::GetComponent(convert_rc_highnode(left), convert_rc_highnode(right))
            }
            HighNode::Dot(left, right) => {
                Node::Dot(convert_rc_highnode(left), convert_rc_highnode(right))
            }
            _ => todo!(),
        }
    }
}
