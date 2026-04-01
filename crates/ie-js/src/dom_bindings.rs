//! DOM bindings for the JavaScript runtime.
//!
//! Registers a `document` global object with methods that operate on a shared
//! `Rc<RefCell<Document>>`, bridging Boa's JS world and the ie-dom arena.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, js_string,
    object::{ObjectInitializer, builtins::JsArray},
    property::Attribute,
};
use ie_dom::{Document, NodeId, NodeKind};

pub type SharedDoc = Rc<RefCell<Document>>;

/// Register the `document` global object on the given JS context.
pub fn register_document(context: &mut Context, doc: SharedDoc) {
    let document_obj = build_document_object(context, doc);
    let _ =
        context.register_global_property(js_string!("document"), document_obj, Attribute::all());
}

fn build_document_object(context: &mut Context, doc: SharedDoc) -> JsValue {
    let doc1 = doc.clone();
    // SAFETY: Rc<RefCell<Document>> contains no boa GC-traced types.
    let get_element_by_id = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let d = doc1.borrow();
            match d.get_element_by_id(d.root, &id) {
                Some(node_id) => Ok(make_element_object(ctx, node_id, &doc1)),
                None => Ok(JsValue::null()),
            }
        })
    };

    let doc2 = doc.clone();
    let create_element = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let tag = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let node_id = doc2.borrow_mut().create_element(&tag);
            Ok(make_element_object(ctx, node_id, &doc2))
        })
    };

    let doc3 = doc.clone();
    let create_text_node = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let node_id = doc3.borrow_mut().create_text(&text);
            Ok(make_element_object(ctx, node_id, &doc3))
        })
    };

    let doc4 = doc.clone();
    let get_elements_by_tag_name = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let tag = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let d = doc4.borrow();
            let ids = d.get_elements_by_tag_name(d.root, &tag);
            let arr = JsArray::new(ctx);
            for id in ids {
                arr.push(make_element_object(ctx, id, &doc4), ctx)?;
            }
            Ok(arr.into())
        })
    };

    let obj = ObjectInitializer::new(context)
        .function(get_element_by_id, js_string!("getElementById"), 1)
        .function(create_element, js_string!("createElement"), 1)
        .function(create_text_node, js_string!("createTextNode"), 1)
        .function(
            get_elements_by_tag_name,
            js_string!("getElementsByTagName"),
            1,
        )
        .build();

    obj.into()
}

/// Create a JS object representing a DOM node, carrying its `NodeId` and
/// methods that operate on the shared document.
fn make_element_object(context: &mut Context, node_id: NodeId, doc: &SharedDoc) -> JsValue {
    let doc1 = doc.clone();
    let nid1 = node_id;
    let get_text_content = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let d = doc1.borrow();
            let text = collect_text_content(&d, nid1);
            Ok(JsValue::from(js_string!(text)))
        })
    };

    let doc2 = doc.clone();
    let nid2 = node_id;
    let set_text_content = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let mut d = doc2.borrow_mut();
            // Remove existing children
            let children: Vec<NodeId> =
                d.node(nid2).map(|n| n.children.clone()).unwrap_or_default();
            for child in children {
                let _ = d.remove_child(nid2, child);
            }
            // Add new text node
            let text_node = d.create_text(&text);
            let _ = d.append_child(nid2, text_node);
            Ok(JsValue::undefined())
        })
    };

    let doc3 = doc.clone();
    let nid3 = node_id;
    let append_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let child_val = args.get_or_undefined(0);
            let child_id = extract_node_id(child_val, ctx)?;
            doc3.borrow_mut()
                .append_child(nid3, child_id)
                .map_err(|e| JsError::from(JsNativeError::typ().with_message(e.to_string())))?;
            Ok(child_val.clone())
        })
    };

    let doc4 = doc.clone();
    let nid4 = node_id;
    let get_attribute = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let d = doc4.borrow();
            match d.get_attribute(nid4, &name) {
                Some(val) => Ok(JsValue::from(js_string!(val.to_string()))),
                None => Ok(JsValue::null()),
            }
        })
    };

    let doc5 = doc.clone();
    let nid5 = node_id;
    let set_attribute = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let name = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let value = args
                .get_or_undefined(1)
                .to_string(ctx)?
                .to_std_string_escaped();
            doc5.borrow_mut().set_attribute(nid5, &name, &value);
            Ok(JsValue::undefined())
        })
    };

    let doc6 = doc.clone();
    let nid6 = node_id;
    let remove_child_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let child_val = args.get_or_undefined(0);
            let child_id = extract_node_id(child_val, ctx)?;
            doc6.borrow_mut()
                .remove_child(nid6, child_id)
                .map_err(|e| JsError::from(JsNativeError::typ().with_message(e.to_string())))?;
            Ok(child_val.clone())
        })
    };

    let doc7 = doc.clone();
    let nid7 = node_id;
    let tag_name = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let d = doc7.borrow();
            match d.node(nid7).and_then(|n| n.element_name()) {
                Some(name) => Ok(JsValue::from(js_string!(name.to_uppercase()))),
                None => Ok(JsValue::null()),
            }
        })
    };

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("__nodeId"),
            JsValue::from(node_id as i32),
            Attribute::empty(),
        )
        .function(get_text_content, js_string!("getTextContent"), 0)
        .function(set_text_content, js_string!("setTextContent"), 1)
        .function(append_child_fn, js_string!("appendChild"), 1)
        .function(remove_child_fn, js_string!("removeChild"), 1)
        .function(get_attribute, js_string!("getAttribute"), 1)
        .function(set_attribute, js_string!("setAttribute"), 2)
        .function(tag_name, js_string!("getTagName"), 0)
        .build();

    obj.into()
}

/// Recursively collect text content from a node and its descendants.
fn collect_text_content(doc: &Document, node_id: NodeId) -> String {
    let Some(node) = doc.node(node_id) else {
        return String::new();
    };
    match &node.kind {
        NodeKind::Text(text) => text.clone(),
        _ => {
            let children = node.children.clone();
            let mut result = String::new();
            for child in children {
                result.push_str(&collect_text_content(doc, child));
            }
            result
        }
    }
}

/// Extract a `NodeId` from a JS element object's `__nodeId` property.
fn extract_node_id(value: &JsValue, ctx: &mut Context) -> JsResult<NodeId> {
    let obj = value.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected a DOM node object"))
    })?;
    let id_val = obj.get(js_string!("__nodeId"), ctx)?;
    let id = id_val.to_number(ctx)? as usize;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JsRuntime;

    fn make_runtime_with_doc(doc: SharedDoc) -> JsRuntime {
        JsRuntime::new_with_document(doc).unwrap()
    }

    #[test]
    fn document_create_element() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc.clone());
        rt.execute("var el = document.createElement('div')")
            .unwrap();
        let result = rt.eval("el.__nodeId >= 0").unwrap();
        assert_eq!(result, "true");
        // Verify it was actually created in the DOM
        let d = doc.borrow();
        assert!(d.node_count() > 1);
    }

    #[test]
    fn document_create_text_node() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc.clone());
        rt.execute("var t = document.createTextNode('hello')")
            .unwrap();
        let result = rt.eval("t.__nodeId >= 0").unwrap();
        assert_eq!(result, "true");
        let d = doc.borrow();
        let node_id = 1; // first node after root
        assert!(d.node(node_id).unwrap().is_text());
    }

    #[test]
    fn document_get_element_by_id_found() {
        let doc = Rc::new(RefCell::new(Document::new()));
        {
            let mut d = doc.borrow_mut();
            let root = d.root;
            let div = d.create_element("div");
            d.set_attribute(div, "id", "test");
            d.append_child(root, div).unwrap();
        }
        let mut rt = make_runtime_with_doc(doc);
        let result = rt.eval("document.getElementById('test') !== null").unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn document_get_element_by_id_not_found() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        let result = rt.eval("document.getElementById('nope')").unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn element_set_get_attribute() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        rt.execute("var el = document.createElement('div'); el.setAttribute('class', 'foo')")
            .unwrap();
        let result = rt.eval("el.getAttribute('class')").unwrap();
        assert_eq!(result, "foo");
    }

    #[test]
    fn element_get_attribute_missing() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        rt.execute("var el = document.createElement('div')")
            .unwrap();
        let result = rt.eval("el.getAttribute('nope')").unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn element_append_child() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc.clone());
        rt.execute(
            "var parent = document.createElement('div'); \
             var child = document.createTextNode('hello'); \
             parent.appendChild(child)",
        )
        .unwrap();
        // Verify in DOM
        let d = doc.borrow();
        // parent is node 1, child is node 2
        let parent_children = d.children(1);
        assert_eq!(parent_children, &[2]);
    }

    #[test]
    fn element_remove_child() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc.clone());
        rt.execute(
            "var parent = document.createElement('div'); \
             var child = document.createElement('span'); \
             parent.appendChild(child); \
             parent.removeChild(child)",
        )
        .unwrap();
        let d = doc.borrow();
        assert!(d.children(1).is_empty());
    }

    #[test]
    fn element_text_content_round_trip() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        rt.execute(
            "var el = document.createElement('p'); \
             el.setTextContent('hello world')",
        )
        .unwrap();
        let result = rt.eval("el.getTextContent()").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn element_text_content_overwrites() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        rt.execute(
            "var el = document.createElement('p'); \
             el.setTextContent('first'); \
             el.setTextContent('second')",
        )
        .unwrap();
        let result = rt.eval("el.getTextContent()").unwrap();
        assert_eq!(result, "second");
    }

    #[test]
    fn element_get_tag_name() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc);
        rt.execute("var el = document.createElement('div')")
            .unwrap();
        let result = rt.eval("el.getTagName()").unwrap();
        assert_eq!(result, "DIV");
    }

    #[test]
    fn document_get_elements_by_tag_name() {
        let doc = Rc::new(RefCell::new(Document::new()));
        {
            let mut d = doc.borrow_mut();
            let root = d.root;
            let div1 = d.create_element("div");
            let span = d.create_element("span");
            let div2 = d.create_element("div");
            d.append_child(root, div1).unwrap();
            d.append_child(root, span).unwrap();
            d.append_child(span, div2).unwrap();
        }
        let mut rt = make_runtime_with_doc(doc);
        let result = rt
            .eval("document.getElementsByTagName('div').length")
            .unwrap();
        assert_eq!(result, "2");
    }

    #[test]
    fn combined_dom_operations() {
        let doc = Rc::new(RefCell::new(Document::new()));
        let mut rt = make_runtime_with_doc(doc.clone());
        rt.execute(
            "var container = document.createElement('div'); \
             container.setAttribute('id', 'app'); \
             var heading = document.createElement('h1'); \
             heading.setTextContent('Hello'); \
             container.appendChild(heading); \
             var para = document.createElement('p'); \
             para.setTextContent('World'); \
             container.appendChild(para)",
        )
        .unwrap();

        // Verify structure in DOM
        let d = doc.borrow();
        // container = 1, heading = 2, text-in-heading = 3, para = 4, text-in-para = 5
        assert_eq!(d.get_attribute(1, "id"), Some("app"));
        assert_eq!(d.children(1), &[2, 4]);
        assert_eq!(d.children(2), &[3]); // heading has text child
        assert_eq!(d.children(4), &[5]); // para has text child
    }
}
