use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;
use rquickjs::{AsyncContext, AsyncRuntime, FromJs};

use crate::storage::{GlobalRoomStorage, RoomMemoryCenter};
use crate::extension::{ExtensionEnv, ExtensionSet, Extension};
use crate::event_loop::EventLoop;
use crate::event_loop::event_loop_impl::EventPortRegistrar;

use super::JsRuntimeBuilder;

pub struct JsRuntime {
    context: AsyncContext,
    event_loop: EventLoop,
    cancel_token: CancellationToken,
}

impl JsRuntime {
    pub(crate) async fn assemble(
        extensions: ExtensionSet,
        cancel_token: CancellationToken,
    ) -> rquickjs::Result<Self> {
        let runtime = AsyncRuntime::new()?;
        let extensions = topological_sort(extensions);

        // ── 事件循环 ─────────────────────────────────────────────
        let mut event_loop = EventLoop::new(runtime.clone(), cancel_token.clone());

        // ── 创建 AsyncContext ────────────────────────────────────
        let ctx = AsyncContext::full(&runtime).await?;

        // 初始化 Context 级公共 userdata
        ctx.with(|js_ctx| {
            js_ctx.store_userdata(RoomMemoryCenter {
                global: Mutex::new(GlobalRoomStorage::new(cancel_token.clone())),
                private_manifest: Mutex::new(HashMap::new()),
            })?;
            Ok::<(), rquickjs::Error>(())
        }).await?;

        // ── 第二层：Extension context setup（异步）───────────────
        for ext in &extensions {
            let mut env = ExtensionEnv::new(
                &runtime,
                &ctx,
                cancel_token.clone(),
                EventPortRegistrar::new(&mut event_loop),
                ext.name(),
            );
            ext.setup(&mut env).await;
        }

        // ── 第三层：JS 胶水模块（惰性注入）──────────────────────
        ctx.with(|js_ctx| {
            for ext in &extensions {
                for specifier in ext.module_specifiers() {
                    if let Some(source) = ext.js_source(specifier) {
                        js_ctx.eval::<(), _>(source)?;
                    }
                }
            }
            Ok::<(), rquickjs::Error>(())
        }).await?;

        Ok(Self { context: ctx, event_loop, cancel_token })
    }

    pub fn builder() -> JsRuntimeBuilder {
        JsRuntimeBuilder::new()
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub async fn eval<R, S>(&self, source: S) -> rquickjs::Result<R>
    where
        R: for<'js> FromJs<'js> + 'static,
        S: Into<Vec<u8>> + Send + 'static,
    {
        self.context.with(|ctx| ctx.eval::<R, _>(source)).await
    }

    pub async fn eval_module<S>(&self, module_name: S, source: S) -> rquickjs::Result<()>
    where
        S: AsRef<str> + Send + 'static,
    {
        let name = module_name.as_ref().to_string();
        let src = source.as_ref().to_string();
        self.context.with(|ctx| {
            let module = rquickjs::Module::declare(ctx, name, src)?;
            let _ = module.eval()?;
            Ok::<(), rquickjs::Error>(())
        }).await?;
        Ok(())
    }

    pub async fn run(mut self) -> rquickjs::Result<()> {
        self.event_loop.run().await
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
