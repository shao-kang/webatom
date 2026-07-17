use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tokio_util::sync::CancellationToken;
use rquickjs::{Context, Runtime, FromJs};

use crate::extension::{ExtensionEnv, ExtensionSet, Extension};
use crate::event_loop::{EventLoop, RenderScheduler};
use crate::module::{ImportMap, setup_module_system};

use super::JsRuntimeBuilder;

pub struct JsRuntime {
    context: Context,
    event_loop: Rc<RefCell<EventLoop>>,
    cancel_token: CancellationToken,
}

impl JsRuntime {
    pub(crate) fn assemble(
        extensions: ExtensionSet,
        cancel_token: CancellationToken,
        render_scheduler: Box<dyn RenderScheduler>,
        import_map: ImportMap,
    ) -> rquickjs::Result<Self> {
        let runtime = Runtime::new()?;
        let extensions = topological_sort(extensions);

        // 为所有扩展的内部 specifier 注入 identity mapping，
        // 使 EsmResolver 能通过 import map 解析 native module 的裸 specifier。
        // 内部 specifier 优先级最高，若用户 import_map 中已有同名条目则先警告再覆盖。
        for ext in &extensions {
            for &spec in ext.native_module_specifiers() {
                if import_map.has(spec) {
                    eprintln!("[webatom] warning: user import map contains reserved internal specifier '{spec}', overriding");
                }
                import_map.insert_internal(spec, spec);
            }
            for (spec, _) in ext.js_modules() {
                if import_map.has(*spec) {
                    eprintln!("[webatom] warning: user import map contains reserved internal specifier '{spec}', overriding");
                }
                import_map.insert_internal(*spec, *spec);
            }
        }

        // 安装模块解析器，resolver 与 import_map 共享同一 Arc
        setup_module_system(&runtime, import_map.clone());

        let context = Context::full(&runtime)?;
        let event_loop = EventLoop::new(runtime.clone(), cancel_token.clone(), render_scheduler, context.clone());
        let event_port_registrar = event_loop.make_registrar();
        let event_loop_rc = Rc::new(RefCell::new(event_loop));
        
        context.with(|js_ctx| {
            js_ctx.store_userdata(event_port_registrar.clone())?;
            Ok::<(), rquickjs::Error>(())
        })?;

        // ctx.with(|js_ctx| {
        //     js_ctx.store_userdata(RoomMemoryCenter {
        //         global: Mutex::new(GlobalRoomStorage::new(cancel_token.clone())),
        //         private_manifest: Mutex::new(HashMap::new()),
        //     })?;
        //     Ok::<(), rquickjs::Error>(())
        // })?;

        for ext in &extensions {
            let mut env = ExtensionEnv::new(
                &context,
                cancel_token.clone(),
                event_port_registrar.clone(),
                ext.name(),
                ext.native_module_specifiers(),
                import_map.clone(),
            );
            ext.native_setup(&mut env);
        }

        context.with(|js_ctx| {
            for ext in &extensions {
                // 用户可 import 的 JS 模块
                for (specifier, source) in ext.js_modules() {
                    let module = rquickjs::Module::declare(js_ctx.clone(), *specifier, *source)?;
                    module.eval()?;
                }
                // globalThis 绑定（以 ESM 执行，可 import 原生模块）
                if let Some(source) = ext.global_js() {
                    let boot_name = format!("@{}-globals", ext.name());
                    let module = rquickjs::Module::declare(js_ctx.clone(), boot_name, source)?;
                    module.eval()?;
                }
            }
            Ok::<(), rquickjs::Error>(())
        })?;

        Ok(Self { context, event_loop: event_loop_rc, cancel_token })
    }

    pub fn builder() -> JsRuntimeBuilder {
        JsRuntimeBuilder::new()
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub fn eval<R, S>(&self, source: S) -> rquickjs::Result<R>
    where
        R: for<'js> FromJs<'js> + 'static,
        S: Into<Vec<u8>>,
    {
        self.context.with(|ctx| ctx.eval::<R, _>(source))
    }

    pub fn eval_module<S>(&self, module_name: S, source: S) -> rquickjs::Result<()>
    where
        S: AsRef<str>,
    {
        let name = module_name.as_ref().to_string();
        let src = source.as_ref().to_string();
        self.context.with(|ctx| {
            let module = rquickjs::Module::declare(ctx, name, src)?;
            let _ = module.eval()?;
            Ok(())
        })
    }

    pub async fn run(&self) -> rquickjs::Result<()> {
        self.event_loop.borrow_mut().run().await
    }
}

// ──────────────────────────────────────────────────────────────
// 拓扑排序（DFS）
// ──────────────────────────────────────────────────────────────

fn topological_sort(extensions: ExtensionSet) -> ExtensionSet {
    let name_to_idx: HashMap<&'static str, usize> = extensions
        .iter()
        .enumerate()
        .map(|(i, e)| (e.name(), i))
        .collect();

    let mut sorted: Vec<usize> = Vec::with_capacity(extensions.len());
    let mut visited: HashSet<usize> = HashSet::new();
    let mut visiting: HashSet<usize> = HashSet::new();

    for i in 0..extensions.len() {
        topo_dfs(i, &extensions, &name_to_idx, &mut visited, &mut visiting, &mut sorted);
    }

    let mut slots: Vec<Option<Box<dyn Extension>>> =
        extensions.into_iter().map(Some).collect();

    sorted.into_iter().map(|i| slots[i].take().unwrap()).collect()
}

fn topo_dfs(
    idx: usize,
    extensions: &ExtensionSet,
    name_to_idx: &HashMap<&'static str, usize>,
    visited: &mut HashSet<usize>,
    visiting: &mut HashSet<usize>,
    sorted: &mut Vec<usize>,
) {
    if visited.contains(&idx) { return; }
    if visiting.contains(&idx) {
        panic!("plugin cycle: `{}`", extensions[idx].name());
    }
    visiting.insert(idx);

    for dep in extensions[idx].depends_on() {
        if let Some(&dep_idx) = name_to_idx.get(dep) {
            topo_dfs(dep_idx, extensions, name_to_idx, visited, visiting, sorted);
        }
    }

    visiting.remove(&idx);
    visited.insert(idx);
    sorted.push(idx);
}
