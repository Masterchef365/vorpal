use crate::*;

pub fn evaluate_node(node: &Node, ctx: &ExternParameters) -> Result<Value, EvalError> {
    fn comp_infix<const N: usize>(
        mut a: [f32; N],
        infix: ComponentInfixOp,
        b: [f32; N],
    ) -> [f32; N] {
        a.iter_mut()
            .zip(&b)
            .for_each(|(a, b)| *a = infix.native(*a, *b));
        a
    }

    fn comp_func<const N: usize>(mut x: [f32; N], func: ComponentFn) -> [f32; N] {
        x.iter_mut().for_each(|x| *x = func.native(*x));
        x
    }

    fn dot(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(a, b)| a * b).sum()
    }

    match node {
        Node::Make(nodes, dtype) => {
            let mut val = Value::default_of_dtype(*dtype);
            let fill = |arr: &mut [f32]| {
                for (node, out) in nodes.iter().zip(arr) {
                    let part = evaluate_node(node, ctx)?;
                    *out = part.try_into()?;
                }
                Ok(())
            };
            match &mut val {
                Value::Scalar(scalar) => {
                    let mut arr = [*scalar];
                    fill(&mut arr)?;
                    *scalar = arr[0];
                }
                Value::Vec2(arr) => fill(arr)?,
                Value::Vec3(arr) => fill(arr)?,
                Value::Vec4(arr) => fill(arr)?,
            }
            Ok(val)
        }
        Node::Constant(value) => Ok(value.clone()),
        Node::ComponentInfixOp(a, op, b) => {
            match (evaluate_node(a, ctx)?, evaluate_node(b, ctx)?) {
                (Value::Scalar(a), Value::Scalar(b)) => {
                    Ok(Value::Scalar(comp_infix([a], *op, [b])[0]))
                }
                (Value::Vec2(a), Value::Vec2(b)) => Ok(Value::Vec2(comp_infix(a, *op, b))),
                (Value::Vec3(a), Value::Vec3(b)) => Ok(Value::Vec3(comp_infix(a, *op, b))),
                (Value::Vec4(a), Value::Vec4(b)) => Ok(Value::Vec4(comp_infix(a, *op, b))),
                _ => Err(EvalError::TypeMismatch),
            }
        }
        Node::ComponentFn(func, a) => match evaluate_node(a, ctx)? {
            Value::Scalar(a) => Ok(Value::Scalar(comp_func([a], *func)[0])),
            Value::Vec2(a) => Ok(Value::Vec2(comp_func(a, *func))),
            Value::Vec3(a) => Ok(Value::Vec3(comp_func(a, *func))),
            Value::Vec4(a) => Ok(Value::Vec4(comp_func(a, *func))),
        },
        Node::GetComponent(value, index) => {
            let value = evaluate_node(value, ctx)?;
            if let Value::Scalar(index) = evaluate_node(index, ctx)? {
                let index = index.clamp(0., value.dtype().n_lanes() as f32);
                let index = (index as usize).clamp(0, value.dtype().n_lanes() - 1);
                Ok(Value::Scalar(match value {
                    Value::Scalar(val) => val,
                    Value::Vec2(arr) => arr[index],
                    Value::Vec3(arr) => arr[index],
                    Value::Vec4(arr) => arr[index],
                }))
            } else {
                Err(EvalError::TypeMismatch)
            }
        }
        // TODO: Typecheck dtype here!
        Node::ExternInput(id, _dtype) => ctx
            .inputs
            .get(id)
            .copied()
            .ok_or_else(|| EvalError::BadInputId(id.clone())),
        Node::Dot(a, b) => match (evaluate_node(a, ctx)?, evaluate_node(b, ctx)?) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a * b)),
            (Value::Vec2(a), Value::Vec2(b)) => Ok(Value::Scalar(dot(&a, &b))),
            (Value::Vec3(a), Value::Vec3(b)) => Ok(Value::Scalar(dot(&a, &b))),
            (Value::Vec4(a), Value::Vec4(b)) => Ok(Value::Scalar(dot(&a, &b))),
            _ => Err(EvalError::TypeMismatch),
        },
    }
}
