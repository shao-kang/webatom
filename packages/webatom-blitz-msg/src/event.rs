use crate::layout::NodeLayout;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    #[serde(rename_all = "camelCase")]
    Click     { node_id: usize, x: f32, y: f32 },
    KeyDown   { key: String, modifiers: u32 },
    KeyUp     { key: String, modifiers: u32 },
    Resize    { width: u32, height: u32 },
    #[serde(rename_all = "camelCase")]
    Focus     { node_id: usize },
    #[serde(rename_all = "camelCase")]
    Blur      { node_id: usize },
    MouseMove { x: f32, y: f32 },
    #[serde(rename_all = "camelCase")]
    MouseDown { node_id: usize, x: f32, y: f32, button: u8 },
    #[serde(rename_all = "camelCase")]
    MouseUp   { node_id: usize, x: f32, y: f32, button: u8 },
    #[serde(rename_all = "camelCase")]
    DblClick  { node_id: usize, x: f32, y: f32 },
    #[serde(rename_all = "camelCase")]
    Scroll    { delta_x: f32, delta_y: f32 },
    /// 异步布局查询结果，对应 DomMsg::QueryLayout
    #[serde(rename_all = "camelCase")]
    LayoutResult(NodeLayout),
    /// nextTick 回调：对应 DomMsg::LayoutNotifyRequest，当前 patch 已应用并完成布局
    LayoutNotify,
}
