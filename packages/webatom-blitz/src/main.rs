use webatom_blitz_msg::{
    DomMsg, DomSnapshot, SnapshotNode, SnapshotNodeData, create_channels,
};

fn main() {
    let (js_side, blitz_side) = create_channels();

    // 模拟 JS 线程：发送一个最简 DOM 树
    std::thread::spawn(move || {
        // 稍等渲染器初始化
        std::thread::sleep(std::time::Duration::from_millis(500));

        let snap = DomSnapshot {
            root: 0,
            nodes: vec![
                SnapshotNode {
                    id: 0,
                    parent: None,
                    children: vec![1],
                    data: SnapshotNodeData::Document,
                },
                SnapshotNode {
                    id: 1,
                    parent: Some(0),
                    children: vec![2],
                    data: SnapshotNodeData::Element {
                        tag: "div".into(),
                        attrs: vec![("style".into(), "padding: 20px; font-size: 24px;".into())],
                    },
                },
                SnapshotNode {
                    id: 2,
                    parent: Some(1),
                    children: vec![],
                    data: SnapshotNodeData::Text {
                        content: "Hello from webAtom!".into(),
                    },
                },
            ],
        };

        let _ = js_side.dom_tx.send(DomMsg::Full(snap));
    });

    // 主线程运行 Blitz（阻塞直到窗口关闭）
    webatom_blitz::run(blitz_side);
}
