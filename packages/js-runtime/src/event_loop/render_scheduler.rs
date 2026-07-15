use tokio::sync::mpsc;

use super::event_loop_impl::EventEnvelope;

// ──────────────────────────────────────────────────────────────
// RenderScheduler
// ──────────────────────────────────────────────────────────────

/// VSync 驱动的渲染调度器接口。
///
/// 实现者负责在合适的时机（真实 VSync 或模拟帧率）向 EventLoop 推送一批 RAF 回调。
/// EventLoop 每轮循环从 `raf_batch_rx` 接收一帧的 `Vec<EventEnvelope>`，一次性 dispatch。
///
/// # 快照语义
///
/// 发送前必须将当前帧 RAF 队列快照（drain 到 Vec），新注册的 rAF 回调不进入本帧。
/// 这与浏览器语义一致：rAF callback 内再调 requestAnimationFrame 会进入下一帧。
pub trait RenderScheduler: Send + 'static {
    /// 将调度器关联到 EventLoop 的收发两端。
    ///
    /// EventLoop 在构造时调用一次；实现者将 `raf_batch_tx` 保存起来，
    /// 在每帧合适时机通过它发送一批 `Vec<EventEnvelope>`。
    /// EventLoop 持有 `raf_batch_rx` 接收端，在主循环中消费。
    fn connect(&mut self, raf_batch_tx: mpsc::UnboundedSender<Vec<EventEnvelope>>);
}

// ──────────────────────────────────────────────────────────────
// HeadlessRenderScheduler（默认实现）
// ──────────────────────────────────────────────────────────────

/// 无头（无渲染）调度器。
///
/// 不驱动任何 VSync，rAF 回调永远不会被发送。
/// 适用于测试、CLI 脚本等无界面场景。
/// 接入真实渲染器（blitz）时替换为对应实现。
pub struct HeadlessRenderScheduler;

impl RenderScheduler for HeadlessRenderScheduler {
    fn connect(&mut self, _raf_batch_tx: mpsc::UnboundedSender<Vec<EventEnvelope>>) {
        // 无头模式：不持有 tx，rAF 批次永远为空
    }
}

impl RenderScheduler for Box<dyn RenderScheduler> {
    fn connect(&mut self, raf_batch_tx: mpsc::UnboundedSender<Vec<EventEnvelope>>) {
        (**self).connect(raf_batch_tx);
    }
}
