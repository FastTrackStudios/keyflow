//! Read-side helpers over [`roxmltree`] that stand in for the lxml
//! `find` / `xpath` calls in the original converter.
//!
//! EnigmaXML places every element in the Finale default namespace and the
//! metadata document in the NotationMetadata namespace; since each document
//! uses a single namespace throughout, we match purely on local element names
//! (the `f:` / `m:` prefixes in the Python XPath are just that one namespace).

use roxmltree::Node;

pub type XNode<'a, 'i> = Node<'a, 'i>;

/// Direct element children of `node` with the given local name.
pub fn children<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Vec<Node<'a, 'i>> {
    node.children()
        .filter(|c| c.is_element() && c.tag_name().name() == name)
        .collect()
}

/// First direct element child with the given local name — `el.find("f:name")`.
pub fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// Whether a direct child element with the given name exists.
pub fn has_child(node: Node, name: &str) -> bool {
    child(node, name).is_some()
}

/// Walk a `/`-style path of local names, taking the first match at each step —
/// `el.find("f:a/f:b/f:c")`.
pub fn find_path<'a, 'i>(node: Node<'a, 'i>, path: &[&str]) -> Option<Node<'a, 'i>> {
    let mut cur = node;
    for name in path {
        cur = child(cur, name)?;
    }
    Some(cur)
}

/// Text of the first path match, if present.
pub fn path_text(node: Node, path: &[&str]) -> Option<String> {
    find_path(node, path).and_then(text)
}

/// An element's text (the text node before its first child), like lxml `.text`.
pub fn text(node: Node) -> Option<String> {
    node.text().map(|s| s.to_string())
}

/// An attribute value.
pub fn attr(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(|s| s.to_string())
}

// ── lookup index ────────────────────────────────────────────────────────
//
// The converter asks the same `/finale/<container>/<name>` questions once per
// staff × measure; walking the tree each time made a 26-staff score take
// ~30 s. Answers are cached as `NodeId`s per document, keyed by the
// document's address plus a generation that `reset_index` bumps, so a later
// document reusing the allocation never sees stale ids.

use roxmltree::NodeId;
use std::cell::RefCell;
use std::collections::HashMap;

type ListKey = (usize, u64, String, String);
type AttrKey = (usize, u64, String, String, String);

thread_local! {
    static GENERATION: RefCell<u64> = const { RefCell::new(0) };
    static LISTS: RefCell<HashMap<ListKey, std::rc::Rc<Vec<NodeId>>>> = RefCell::new(HashMap::new());
    static BY_ATTR: RefCell<HashMap<AttrKey, std::rc::Rc<HashMap<String, NodeId>>>> =
        RefCell::new(HashMap::new());
}

/// Drop every cached lookup. Call before converting a new document.
pub fn reset_index() {
    GENERATION.with(|g| *g.borrow_mut() += 1);
    LISTS.with(|l| l.borrow_mut().clear());
    BY_ATTR.with(|l| l.borrow_mut().clear());
}

fn doc_key(root: Node) -> (usize, u64) {
    let addr = std::ptr::from_ref(root.document()) as usize;
    (addr, GENERATION.with(|g| *g.borrow()))
}

fn grandchild_ids(root: Node, container: &str, name: &str) -> std::rc::Rc<Vec<NodeId>> {
    let (addr, generation) = doc_key(root);
    let key = (addr, generation, container.to_string(), name.to_string());
    if let Some(hit) = LISTS.with(|l| l.borrow().get(&key).cloned()) {
        return hit;
    }
    let mut ids = Vec::new();
    for c in children(root, container) {
        ids.extend(children(c, name).into_iter().map(|n| n.id()));
    }
    let ids = std::rc::Rc::new(ids);
    LISTS.with(|l| l.borrow_mut().insert(key, ids.clone()));
    ids
}

/// All `container/name` grandchildren under `root` — the common
/// `/f:finale/f:<container>/f:<name>` XPath shape.
pub fn grandchildren<'a, 'i>(root: Node<'a, 'i>, container: &str, name: &str) -> Vec<Node<'a, 'i>> {
    let doc = root.document();
    grandchild_ids(root, container, name)
        .iter()
        .filter_map(|id| doc.get_node(*id))
        .collect()
}

/// First `container/name` grandchild whose `attr_name` equals `attr_val`.
pub fn find_by_attr<'a, 'i>(
    root: Node<'a, 'i>,
    container: &str,
    name: &str,
    attr_name: &str,
    attr_val: &str,
) -> Option<Node<'a, 'i>> {
    let (addr, generation) = doc_key(root);
    let key = (
        addr,
        generation,
        container.to_string(),
        name.to_string(),
        attr_name.to_string(),
    );
    let map = match BY_ATTR.with(|m| m.borrow().get(&key).cloned()) {
        Some(map) => map,
        None => {
            let doc = root.document();
            let mut map: HashMap<String, NodeId> = HashMap::new();
            for id in grandchild_ids(root, container, name).iter() {
                if let Some(node) = doc.get_node(*id)
                    && let Some(v) = node.attribute(attr_name)
                {
                    // First match wins, as the linear search did.
                    map.entry(v.to_string()).or_insert(*id);
                }
            }
            let map = std::rc::Rc::new(map);
            BY_ATTR.with(|m| m.borrow_mut().insert(key, map.clone()));
            map
        }
    };
    map.get(attr_val)
        .and_then(|id| root.document().get_node(*id))
}
