use vorpal_wasm::CodeAnalysis;
use anyhow::Result;
use std::rc::Rc;
use vorpal_core::*;
use wasm_bridge::*;

// TODO:
// Change Value to something like VectorValue<T, const N: usize>([T; N]);
// * Other datatypes
// * Longer vectors(?) - go by powers of two; octonions!

/*
pub fn evaluate_node(node: &Node, ctx: &ExternContext) -> Result<Value> {
    let mut engine = Engine::new().unwrap();
    engine.eval(node, ctx)
}
*/

pub struct Engine {
    wasm_engine: wasm_bridge::Engine,
    pub cache: Option<CachedCompilation>,
}

pub struct CachedCompilation {
    pub node: Node,
    pub instance: Instance,
    pub store: Store<()>,
    pub mem: Memory,
    pub anal: CodeAnalysis,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            wasm_engine: wasm_bridge::Engine::new(&Default::default())?,
            cache: None,
        })
    }

    /*
    pub fn eval(&mut self, node: &Node, ctx: &ExternContext) -> Result<Value> {
        // Generate input list in random order
        let input_list = ctx
            .inputs()
            .iter()
            .map(|(name, value)| (name.clone(), value.dtype()))
            .collect::<Vec<(ExternInputId, DataType)>>();

        let mut store = Store::new(&self.wasm_engine, ());
        let (instance, analysis) = self.compile(node, input_list)?;
        self.exec_instance(&analysis, &instance, &mut store, ctx)
    }
    */

    pub fn eval_image(&mut self, node: &Node, ctx: &ExternContext) -> Result<Vec<f32>> {
        let res_key = &ExternInputId::new(crate::RESOLUTION_KEY.into());
        let time_key = &ExternInputId::new(crate::TIME_KEY.into());
        let pos_key = &ExternInputId::new(crate::POS_KEY.into());

        let input_list = vec![
            // See vorpal-wasm-builtins' special_image_function
            (res_key.clone(), DataType::Vec2),
            (pos_key.clone(), DataType::Vec2),
            (time_key.clone(), DataType::Scalar),
        ];

        let Value::Vec2([width, height]) = ctx.inputs()[&res_key] else {
            panic!("Wrong vector type")
        };
        let Value::Scalar(time) = ctx.inputs()[&time_key] else {
            panic!("Wrong vector type")
        };
        let width = width as u32;
        let height = height as u32;

        let mut compile_data: CachedCompilation = self
            .cache
            .take()
            .filter(|cache| &cache.node == node)
            .map(|cache| Ok(cache))
            .unwrap_or_else(|| -> anyhow::Result<CachedCompilation> {
                let mut store = Store::new(&self.wasm_engine, ());

                // Compile code
                let (kernel_module, anal) = self.compile(node, input_list)?;

                // Start linking modules
                let mut linker = Linker::new(&mut self.wasm_engine);

                // Create a memory which all modules know to import
                let memory_ty = MemoryType::new(100, None);
                let mem = Memory::new(&mut store, memory_ty)?;
                // Gleaned from compiling Rust to WAST and adding the 
                // `rustflags = ["-C", "link-args=--import-memory"]`
                // to .cargo/config.toml
                linker.define(&store, "env", "memory", mem)?;

                // Add modules
                linker.module(&mut store, "builtins", &self.builtins_module()?)?;
                linker.module(&mut store, "kernel", &kernel_module)?;
                let instance = linker.instantiate(&mut store, &self.image_module()?)?;

                Ok(CachedCompilation {
                    node: node.clone(),
                    instance,
                    store,
                    mem,
                    anal,
                })
            })?;

        let func = compile_data
            .instance
            .get_typed_func::<(u32, u32, f32), u32>(&mut compile_data.store, "make_image")?;

        let ptr = func.call(&mut compile_data.store, (width, height, time))?;

        let mut out_image = vec![0_f32; (width * height * 4) as usize];
        compile_data.mem.read(
            &mut compile_data.store,
            ptr as usize,
            bytemuck::cast_slice_mut(&mut out_image),
        )?;

        self.cache = Some(compile_data);

        //dbg!(&out_image);

        Ok(out_image)
    }

    fn builtins_module(&self) -> Result<Module> {
        Ok(Module::new(&self.wasm_engine, vorpal_wasm::BUILTINS_WASM)?)
    }

    fn image_module(&self) -> Result<Module> {
        let builtins_wasm =
            include_bytes!("../../target/wasm32-unknown-unknown/release/vorpal_image.wasm");
        Ok(Module::new(&self.wasm_engine, builtins_wasm)?)
    }

    fn compile(
        &self,
        node: &Node,
        input_list: Vec<(ExternInputId, DataType)>,
    ) -> Result<(Module, CodeAnalysis)> {
        let analysis = CodeAnalysis::new(Rc::new(node.clone()), input_list);
        let wat = analysis.compile_to_wat()?;
        let kernel_module = Module::new(&self.wasm_engine, wat)?;
        Ok((kernel_module, analysis))
    }

    /*
    fn exec_instance(
        &mut self,
        analysis: &CodeAnalysis,
        kernel_module: &Module,
        mut store: &mut Store<()>,
        ctx: &ExternContext,
    ) -> Result<Value> {
        let mut linker = Linker::new(&mut self.wasm_engine);
        linker.module(&mut store, "builtins", &self.builtins_module()?)?;
        let instance = linker.instantiate(&mut store, &kernel_module)?;

        let kernel = instance
            .get_func(&mut store, "kernel")
            .ok_or_else(|| anyhow::format_err!("Kernel function not found"))?;

        // Create parameter list
        let mut params = vec![];
        for (name, _dtype) in analysis.input_list() {
            let input_val = ctx.inputs()[name];
            params.extend(
                input_val
                    .iter_vector_floats()
                    .map(|f| Val::F32(f.to_bits())),
            );
        }

        // Create output list
        let mut results = vec![];
        let output_dtype = analysis.final_output_dtype();
        results.extend((0..output_dtype.n_lanes()).map(|_| Val::F32(0_f32.to_bits())));

        // Call the function
        kernel.call(&mut store, &params, &mut results)?;

        // Unwind the results
        Ok(match output_dtype {
            DataType::Scalar => Value::Scalar(results[0].f32().unwrap()),
            DataType::Vec2 => {
                let mut val = [0.; 2];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec2(val)
            }
            DataType::Vec3 => {
                let mut val = [0.; 3];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec3(val)
            }
            DataType::Vec4 => {
                let mut val = [0.; 4];
                val.iter_mut()
                    .zip(&results)
                    .for_each(|(v, res)| *v = res.f32().unwrap());
                Value::Vec4(val)
            }
        })
    }
    */
}
