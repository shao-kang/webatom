use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{class::Trace, Class, Ctx, Function, Object, Result};

use crate::core::Document;

#[derive(Trace)]
#[rquickjs::class(rename = "NodeHandle")]
pub struct NodeHandle {
    #[qjs(skip_trace)]
    pub id: usize,
    #[qjs(skip_trace)]
    pub doc: Rc<RefCell<Document>>,
}

unsafe impl<'js> rquickjs::JsLifetime<'js> for NodeHandle {
    type Changed<'to> = NodeHandle;
}

#[rquickjs::methods]
impl NodeHandle {}

impl Drop for NodeHandle {
    fn drop(&mut self) {
        self.doc.borrow_mut().dec_handle(self.id);
    }
}

fn make_handle<'js>(
    ctx: Ctx<'js>,
    id: usize,
    doc: &Rc<RefCell<Document>>,
) -> Result<Class<'js, NodeHandle>> {
    doc.borrow_mut().inc_handle(id);
    Class::instance(ctx, NodeHandle { id, doc: doc.clone() })
}

pub fn init(ctx: &Ctx<'_>, doc: Rc<RefCell<Document>>) -> Result<()> {
    Class::<NodeHandle>::define(&ctx.globals())?;

    let ns = Object::new(ctx.clone())?;

    // -- 节点创建 --
    {
        let d = doc.clone();
        ns.set(
            "createElement",
            Function::new(ctx.clone(), move |ctx: Ctx<'_>, tag: String| {
                let id = d.borrow_mut().create_element(&tag);
                make_handle(ctx, id, &d)
            })?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "createTextNode",
            Function::new(ctx.clone(), move |ctx: Ctx<'_>, content: String| {
                let id = d.borrow_mut().create_text_node(&content);
                make_handle(ctx, id, &d)
            })?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "createComment",
            Function::new(ctx.clone(), move |ctx: Ctx<'_>, content: String| {
                let id = d.borrow_mut().create_comment(&content);
                make_handle(ctx, id, &d)
            })?,
        )?;
    }

    // -- 文档根节点 --
    {
        let d = doc.clone();
        ns.set(
            "documentElement",
            Function::new(ctx.clone(), move |ctx: Ctx<'_>| {
                let id = d.borrow().root();
                make_handle(ctx, id, &d)
            })?,
        )?;
    }

    // -- 树操作 --
    {
        let d = doc.clone();
        ns.set(
            "appendChild",
            Function::new(
                ctx.clone(),
                move |parent: Class<'_, NodeHandle>, child: Class<'_, NodeHandle>| -> Result<()> {
                    let parent_id = parent.borrow().id;
                    let child_id = child.borrow().id;
                    d.borrow_mut().append_child(parent_id, child_id);
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "removeChild",
            Function::new(
                ctx.clone(),
                move |parent: Class<'_, NodeHandle>, child: Class<'_, NodeHandle>| -> Result<()> {
                    let parent_id = parent.borrow().id;
                    let child_id = child.borrow().id;
                    d.borrow_mut().remove_child(parent_id, child_id);
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "insertBefore",
            Function::new(
                ctx.clone(),
                move |parent: Class<'_, NodeHandle>,
                      new_node: Class<'_, NodeHandle>,
                      ref_node: Class<'_, NodeHandle>|
                      -> Result<()> {
                    let parent_id = parent.borrow().id;
                    let new_id = new_node.borrow().id;
                    let ref_id = ref_node.borrow().id;
                    d.borrow_mut().insert_before(parent_id, new_id, ref_id);
                    Ok(())
                },
            )?,
        )?;
    }

    // -- 节点关系查询 --
    {
        let d = doc.clone();
        ns.set(
            "parentNode",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, node: Class<'_, NodeHandle>| -> Result<Option<Class<'_, NodeHandle>>> {
                    let id = node.borrow().id;
                    d.borrow().parent_node(id).map(|pid| make_handle(ctx, pid, &d)).transpose()
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "firstChild",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, node: Class<'_, NodeHandle>| -> Result<Option<Class<'_, NodeHandle>>> {
                    let id = node.borrow().id;
                    d.borrow().first_child(id).map(|cid| make_handle(ctx, cid, &d)).transpose()
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "lastChild",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, node: Class<'_, NodeHandle>| -> Result<Option<Class<'_, NodeHandle>>> {
                    let id = node.borrow().id;
                    d.borrow().last_child(id).map(|cid| make_handle(ctx, cid, &d)).transpose()
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "nextSibling",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, node: Class<'_, NodeHandle>| -> Result<Option<Class<'_, NodeHandle>>> {
                    let id = node.borrow().id;
                    d.borrow().next_sibling(id).map(|sid| make_handle(ctx, sid, &d)).transpose()
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "previousSibling",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, node: Class<'_, NodeHandle>| -> Result<Option<Class<'_, NodeHandle>>> {
                    let id = node.borrow().id;
                    d.borrow().previous_sibling(id).map(|sid| make_handle(ctx, sid, &d)).transpose()
                },
            )?,
        )?;
    }

    // -- 节点信息 --
    {
        let d = doc.clone();
        ns.set(
            "nodeType",
            Function::new(ctx.clone(), move |node: Class<'_, NodeHandle>| -> Result<u16> {
                Ok(d.borrow().node_type(node.borrow().id).unwrap_or(0))
            })?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "tagName",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>| -> Result<Option<String>> {
                    Ok(d.borrow().tag_name(node.borrow().id))
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "nodeValue",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>| -> Result<Option<String>> {
                    Ok(d.borrow().node_value(node.borrow().id))
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "setNodeValue",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>, value: String| -> Result<()> {
                    let id = node.borrow().id;
                    d.borrow_mut().set_node_value(id, &value);
                    Ok(())
                },
            )?,
        )?;
    }

    // -- 元素属性 --
    {
        let d = doc.clone();
        ns.set(
            "getAttribute",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>, name: String| -> Result<Option<String>> {
                    let id = node.borrow().id;
                    Ok(d.borrow().get_attribute(id, &name))
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "setAttribute",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>, name: String, value: String| -> Result<()> {
                    let id = node.borrow().id;
                    d.borrow_mut().set_attribute(id, &name, &value);
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let d = doc.clone();
        ns.set(
            "removeAttribute",
            Function::new(
                ctx.clone(),
                move |node: Class<'_, NodeHandle>, name: String| -> Result<()> {
                    let id = node.borrow().id;
                    d.borrow_mut().remove_attribute(id, &name);
                    Ok(())
                },
            )?,
        )?;
    }

    // -- GC --
    {
        let d = doc.clone();
        ns.set(
            "tickGc",
            Function::new(ctx.clone(), move || -> Result<()> {
                d.borrow_mut().tick_gc();
                Ok(())
            })?,
        )?;
    }

    ctx.globals().set("__web_atom_dom", ns)?;
    Ok(())
}
