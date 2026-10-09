//! Child lists of the headless DOM. DocHost answers parent and next-sibling queries but has no child list, so the lists
//! are rebuilt from the node table: HostNode(n) is dense (a node never reuses its slot), and a node's parent is one query
//! away. A probe node finds the end of the table.

use std::collections::{HashMap, HashSet};

use vn_runtime::host::{Host, HostNode};
use vn_runtime::host_doc::DocHost;

/// The ordered children of every node that has one.
pub struct Index {
    kids: HashMap<HostNode, Vec<HostNode>>,
}

impl Index {
    /// Scans the whole node table. Fails only when the sibling links of a parent do not form one chain (a DOM bug).
    pub fn build(host: &DocHost) -> Result<Index, String> {
        let mut h = host.clone();
        let probe = h.create_comment("");
        h.release(probe);
        let mut raw: HashMap<HostNode, Vec<HostNode>> = HashMap::new();
        for n in 0..probe.0 {
            let node = HostNode(n);
            if let Some(p) = h.parent_node(node) {
                raw.entry(p).or_default().push(node);
            }
        }
        let mut kids = HashMap::with_capacity(raw.len());
        for (parent, list) in raw {
            let nexts: HashSet<HostNode> = list.iter().filter_map(|&c| h.next_sibling(c)).collect();
            let firsts: Vec<HostNode> = list.iter().copied().filter(|c| !nexts.contains(c)).collect();
            let [first] = firsts[..] else {
                return Err(format!("child list of node {} has no single first child", parent.0));
            };
            let mut ordered = Vec::with_capacity(list.len());
            let mut cur = Some(first);
            while let Some(c) = cur {
                if ordered.len() == list.len() {
                    return Err(format!("sibling chain of node {} is longer than its child list", parent.0));
                }
                ordered.push(c);
                cur = h.next_sibling(c);
            }
            if ordered.len() != list.len() {
                return Err(format!("sibling chain of node {} does not reach every child", parent.0));
            }
            kids.insert(parent, ordered);
        }
        Ok(Index { kids })
    }

    /// The children of `node` in document order (text and comment nodes included).
    pub fn children(&self, node: HostNode) -> &[HostNode] {
        self.kids.get(&node).map(Vec::as_slice).unwrap_or(&[])
    }
}
