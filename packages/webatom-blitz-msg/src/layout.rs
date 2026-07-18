#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeLayout {
    pub node_id: usize,
    /// 相对于视口的位置与尺寸（对应 getBoundingClientRect）
    pub x:            f32,
    pub y:            f32,
    pub width:        f32,
    pub height:       f32,
    /// 元素 scrollLeft / scrollTop
    pub scroll_left:  f32,
    pub scroll_top:   f32,
    /// clientWidth / clientHeight（content + padding，不含滚动条）
    pub client_width:  f32,
    pub client_height: f32,
}
