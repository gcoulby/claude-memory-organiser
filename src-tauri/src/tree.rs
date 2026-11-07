//! Message trees. Editing a prompt or retrying a reply creates a sibling
//! branch, so a conversation is a tree, not a list.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Node {
    pub uuid: String,
    pub parent: Option<String>,
    pub created_at: String,
    pub array_idx: i64,
}

impl Node {
    fn order_key(&self) -> (&str, i64) {
        (self.created_at.as_str(), self.array_idx)
    }
}

pub struct Tree<'a> {
    nodes: &'a [Node],
    by_uuid: HashMap<&'a str, usize>,
    /// Children of each node, and of the virtual root under `None`, oldest first.
    children: HashMap<Option<usize>, Vec<usize>>,
}

impl<'a> Tree<'a> {
    pub fn new(nodes: &'a [Node]) -> Self {
        let by_uuid: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.uuid.as_str(), i))
            .collect();
        let mut children: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            // A parent missing from the export makes the message a root.
            let parent = n.parent.as_deref().and_then(|p| by_uuid.get(p).copied());
            children.entry(parent).or_default().push(i);
        }
        for list in children.values_mut() {
            list.sort_by(|&a, &b| nodes[a].order_key().cmp(&nodes[b].order_key()));
        }
        Self {
            nodes,
            by_uuid,
            children,
        }
    }

    pub fn index_of(&self, uuid: &str) -> Option<usize> {
        self.by_uuid.get(uuid).copied()
    }

    fn parent_of(&self, i: usize) -> Option<usize> {
        self.nodes[i]
            .parent
            .as_deref()
            .and_then(|p| self.by_uuid.get(p).copied())
    }

    /// The node and its siblings (including itself), oldest first.
    pub fn siblings(&self, i: usize) -> &[usize] {
        self.children
            .get(&self.parent_of(i))
            .map_or(&[], Vec::as_slice)
    }

    /// The path from a root to the most recent leaf below `via`, as indices,
    /// root first. With no `via`, this is the main path: the export does not
    /// say which branch was current, so the most recent leaf overall wins.
    pub fn path_through(&self, via: Option<usize>) -> Vec<usize> {
        let starts: Vec<usize> = match via {
            Some(v) => vec![v],
            None => self.children.get(&None).cloned().unwrap_or_default(),
        };
        let leaf = self.latest_leaf_below(&starts);

        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut current = leaf;
        while let Some(i) = current {
            // Guards against a cycle in malformed data.
            if !seen.insert(i) {
                break;
            }
            path.push(i);
            current = self.parent_of(i);
        }
        path.reverse();
        path
    }

    fn latest_leaf_below(&self, starts: &[usize]) -> Option<usize> {
        let mut stack = starts.to_vec();
        let mut seen = HashSet::new();
        let mut best: Option<usize> = None;
        while let Some(i) = stack.pop() {
            if !seen.insert(i) {
                continue;
            }
            match self.children.get(&Some(i)) {
                Some(kids) if !kids.is_empty() => stack.extend(kids),
                _ => {
                    if best.is_none_or(|b| self.nodes[i].order_key() > self.nodes[b].order_key()) {
                        best = Some(i);
                    }
                }
            }
        }
        // Everything was in a cycle: fall back to the most recent node.
        best.or_else(|| {
            starts
                .iter()
                .copied()
                .max_by(|&a, &b| self.nodes[a].order_key().cmp(&self.nodes[b].order_key()))
        })
    }
}

/// The main path, as indices into `nodes`, root first.
pub fn main_path(nodes: &[Node]) -> Vec<usize> {
    Tree::new(nodes).path_through(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(uuid: &str, parent: Option<&str>, at: &str) -> Node {
        Node {
            uuid: uuid.into(),
            parent: parent.map(Into::into),
            created_at: at.into(),
            array_idx: 0,
        }
    }

    fn uuids(nodes: &[Node], path: &[usize]) -> Vec<String> {
        path.iter().map(|&i| nodes[i].uuid.clone()).collect()
    }

    /// m1 -> m2 -> { m3a (older) -> m4a, m3b (newer) }, plus an older root r0.
    fn branchy() -> Vec<Node> {
        vec![
            node("r0", None, "0"),
            node("m1", None, "1"),
            node("m2", Some("m1"), "2"),
            node("m3a", Some("m2"), "3"),
            node("m4a", Some("m3a"), "4"),
            node("m3b", Some("m2"), "5"),
        ]
    }

    #[test]
    fn main_path_ends_at_the_newest_leaf() {
        let nodes = branchy();
        assert_eq!(uuids(&nodes, &main_path(&nodes)), ["m1", "m2", "m3b"]);
    }

    #[test]
    fn path_through_a_branch_follows_it_to_its_newest_leaf() {
        let nodes = branchy();
        let tree = Tree::new(&nodes);
        let via = tree.index_of("m3a").expect("m3a");
        assert_eq!(
            uuids(&nodes, &tree.path_through(Some(via))),
            ["m1", "m2", "m3a", "m4a"]
        );
        let root = tree.index_of("r0").expect("r0");
        assert_eq!(uuids(&nodes, &tree.path_through(Some(root))), ["r0"]);
    }

    #[test]
    fn siblings_are_ordered_oldest_first() {
        let nodes = branchy();
        let tree = Tree::new(&nodes);
        let m3b = tree.index_of("m3b").expect("m3b");
        assert_eq!(uuids(&nodes, tree.siblings(m3b)), ["m3a", "m3b"]);
        let m1 = tree.index_of("m1").expect("m1");
        assert_eq!(uuids(&nodes, tree.siblings(m1)), ["r0", "m1"]);
    }

    #[test]
    fn survives_a_cycle() {
        let nodes = vec![
            node("a", Some("b"), "1"),
            node("b", Some("a"), "2"),
            node("c", Some("b"), "3"),
        ];
        let path = main_path(&nodes);
        assert!(path.len() <= 3);
        let tree = Tree::new(&nodes);
        assert!(tree.path_through(tree.index_of("a")).len() <= 3);
    }
}
