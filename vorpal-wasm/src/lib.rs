use anyhow::Result;
use vorpal_core::*;
use wasm_bridge::*;

pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    Engine::new()?.eval(&node, ctx)
}

pub struct Engine {
    wasm_engine: wasm_bridge::Engine,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            wasm_engine: wasm_bridge::Engine::new(&Default::default())?,
        })
    }

    pub fn eval(&mut self, node: &Node, ctx: &ExternContext) -> Result<Value> {
        let mut codegen = CodeGenerator::new(ctx.inputs().keys().cloned().collect());

        let wat = codegen.compile_to_wat(node)?;
        let module = Module::new(&self.wasm_engine, wat)?;
        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        self.exec_instance(&codegen, &instance, &mut store, ctx)
    }

    fn exec_instance(
        &mut self,
        codegen: &CodeGenerator,
        instance: &Instance,
        mut store: &mut Store<()>,
        ctx: &ExternContext,
    ) -> Result<Value> {
        let kernel = instance.get_typed_func::<(f32, f32), (f32, f32)>(&mut store, "kernel")?;
        Ok(Value::Vec2(Vec2::from(
            kernel.call(&mut store, (2.5, 5.0))?,
        )))
    }
}

/// Denotes the "name" of a local variable; e.g. local.get 9
type LocalVarId = u32;

/// Compile a node into its equivalent
#[derive(Default)]
struct CodeGenerator {
    locals: HashMap<HashRcByPtr<Node>, LocalVarId>,
    inputs: HashMap<ExternInputId, LocalVarId>,
    explored: HashSet<HashRcByPtr<Node>>,
    next_var_id: LocalVarId,
}

impl CodeGenerator {
    pub fn new(input_names: Vec<ExternInputId>) -> Self {
        // Parameter list 
        let mut inputs = HashMap::new();
        let mut next_var_id = 0;
        for name in input_names {
            inputs.insert(name, next_var_id);
            next_var_id += 1;
        }

        Self {
            next_var_id,
            ..Default::default()
        }
    }

    pub fn compile_to_wat(&mut self, node: &Node) -> Result<String> {
        self.find_inputs_and_locals_recursive(Rc::new(node.clone()));
        dbg!(&self.inputs);
        dbg!(&self.explored.len());

        let param_list_text = "f32 f32";
        let result_list_text = "f32 f32";
        let function_body_text = "
    local.get 0
    local.get 1
    f32.sub
    local.get 0
    local.get 1
    f32.add
    ";

        let module_text = format!(
            r#"(module
  (func $kernel (param {param_list_text}) (result {result_list_text})
{function_body_text}
  )
  (export "kernel" (func $kernel))
  (memory (;0;) 16)
  (export "memory" (memory 0))
)"#
        );

        //println!("{}", module_text);

        Ok(module_text)
    }

    fn find_inputs_and_locals_recursive(&mut self, node: Rc<Node>) {
        if !self.explored.insert(HashRcByPtr(node.clone())) {
            return;
        }

        match &*node {
            Node::ExternInput(name) => {
                let id = self.gen_var_id();
                self.inputs.insert(name.clone(), id);
                self.locals.insert(HashRcByPtr(node.clone()), id);
            }
            // Depth-first search
            Node::Dot(a, b) | Node::ComponentInfixOp(a, _, b) | Node::GetComponent(a, b) => {
                self.find_inputs_and_locals_recursive(a.clone());
                self.find_inputs_and_locals_recursive(b.clone());
            }
            Node::ExternSampler(_) => todo!(),
            Node::Constant(_) => (),
            Node::Make(sub_nodes, _) => for sub_node in sub_nodes {
                self.find_inputs_and_locals_recursive(sub_node.clone());
            }
            Node::ComponentFn(_, a) => {
                self.find_inputs_and_locals_recursive(a.clone());
            }
        }
    }

    fn gen_var_id(&mut self) -> LocalVarId {
        let ret = self.next_var_id;
        self.next_var_id += 1;
        ret
    }

    fn compile_to_wat_recursive(&mut self, node: &Node) -> Result<String> {
        todo!()
    }
}

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Clone, Default)]
struct HashRcByPtr<T>(pub Rc<T>);

impl<T> Hash for HashRcByPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state)
    }
}

impl<T> Eq for HashRcByPtr<T> {}

impl<T> PartialEq for HashRcByPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::as_ptr(&self.0).eq(&Rc::as_ptr(&other.0))
    }
}
