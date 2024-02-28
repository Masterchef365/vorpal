use std::rc::Rc;

use crate::{Node, DataType, ExternInputId, Value, ComponentInfixOp, ComponentFn};

/// A higher-level, nicer set of nodes. Compiles to the lower-level set ...
/// HighNode is a strict superset of Node. Is always directly convertible to Node.
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

pub enum Swizzle {

}
