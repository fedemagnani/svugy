use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// Carry a node's size in, and its rank, order and centre through the pipeline.
#[derive(Debug, Clone, Default)]
pub(crate) struct NodeLabel {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) x: Option<f64>,
    pub(crate) y: Option<f64>,
    pub(crate) rank: Option<i32>,
    pub(crate) order: Option<i32>,
    pub(crate) is_dummy: bool,
    pub(crate) low: Option<i32>,
    pub(crate) lim: Option<i32>,
    pub(crate) parent_node: Option<String>,
}

/// Carry an edge's ranking constraints and its network-simplex cut value.
#[derive(Debug, Clone, Default)]
pub(crate) struct EdgeLabel {
    pub(crate) minlen: Option<i32>,
    pub(crate) weight: Option<f64>,
    pub(crate) cutvalue: Option<f64>,
}

/// Identify an edge by its endpoints and, for an edge parallel to another, a name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Edge {
    pub(crate) v: String,
    pub(crate) w: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone)]
struct EdgeEntry {
    v: String,
    w: String,
    name: Option<String>,
    label: EdgeLabel,
}

/// Store labelled nodes and named edges in insertion order, as dagre's graphlib does.
pub(crate) struct Graph {
    is_directed: bool,
    nodes: IndexMap<String, NodeLabel>,
    edges: IndexMap<String, EdgeEntry>,
    in_edges: HashMap<String, IndexMap<String, Vec<String>>>,
    out_edges: HashMap<String, IndexMap<String, Vec<String>>>,
    unique_id_counter: u64,
}

impl Graph {
    pub(crate) fn directed() -> Self {
        Self::with_directedness(true)
    }

    pub(crate) fn undirected() -> Self {
        Self::with_directedness(false)
    }

    fn with_directedness(is_directed: bool) -> Self {
        Graph {
            is_directed,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            in_edges: HashMap::new(),
            out_edges: HashMap::new(),
            unique_id_counter: 0,
        }
    }

    /// Generate a string id with the given prefix, distinct from every id this graph generated before.
    pub(crate) fn unique_id(&mut self, prefix: &str) -> String {
        self.unique_id_counter += 1;
        format!("{}{}", prefix, self.unique_id_counter)
    }

    pub(crate) fn set_node(&mut self, v: &str, label: NodeLabel) {
        if let Some(existing) = self.nodes.get_mut(v) {
            *existing = label;
            return;
        }
        self.nodes.insert(v.to_string(), label);
        self.in_edges.insert(v.to_string(), IndexMap::new());
        self.out_edges.insert(v.to_string(), IndexMap::new());
    }

    pub(crate) fn node(&self, v: &str) -> &NodeLabel {
        self.nodes
            .get(v)
            .unwrap_or_else(|| panic!("node '{v}' not found"))
    }

    pub(crate) fn node_mut(&mut self, v: &str) -> &mut NodeLabel {
        self.nodes
            .get_mut(v)
            .unwrap_or_else(|| panic!("node '{v}' not found"))
    }

    pub(crate) fn node_opt(&self, v: &str) -> Option<&NodeLabel> {
        self.nodes.get(v)
    }

    pub(crate) fn has_node(&self, v: &str) -> bool {
        self.nodes.contains_key(v)
    }

    pub(crate) fn nodes(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn sources(&self) -> Vec<String> {
        self.nodes
            .keys()
            .filter(|v| self.in_edges.get(*v).is_none_or(|m| m.is_empty()))
            .cloned()
            .collect()
    }

    /// Insert the edge, creating missing endpoints, or replace the label of the edge with the same endpoints and name.
    pub(crate) fn set_edge(&mut self, v: &str, w: &str, label: EdgeLabel, name: Option<&str>) {
        let key = self.edge_key(v, w, name);
        if !self.has_node(v) {
            self.set_node(v, NodeLabel::default());
        }
        if !self.has_node(w) {
            self.set_node(w, NodeLabel::default());
        }
        if let Some(existing) = self.edges.get_mut(&key) {
            existing.label = label;
            return;
        }
        let entry = EdgeEntry {
            v: v.to_string(),
            w: w.to_string(),
            name: name.map(str::to_string),
            label,
        };
        self.edges.insert(key.clone(), entry);
        Self::link(&mut self.out_edges, v, w, &key);
        Self::link(&mut self.in_edges, w, v, &key);
        if !self.is_directed {
            Self::link(&mut self.out_edges, w, v, &key);
            Self::link(&mut self.in_edges, v, w, &key);
        }
    }

    pub(crate) fn edge(&self, e: &Edge) -> Option<&EdgeLabel> {
        let key = self.edge_key(&e.v, &e.w, e.name.as_deref());
        self.edges.get(&key).map(|entry| &entry.label)
    }

    pub(crate) fn edge_vw(&self, v: &str, w: &str) -> Option<&EdgeLabel> {
        let key = self.edge_key(v, w, None);
        self.edges.get(&key).map(|entry| &entry.label)
    }

    pub(crate) fn edge_vw_mut(&mut self, v: &str, w: &str) -> Option<&mut EdgeLabel> {
        let key = self.edge_key(v, w, None);
        self.edges.get_mut(&key).map(|entry| &mut entry.label)
    }

    pub(crate) fn has_edge(&self, v: &str, w: &str) -> bool {
        self.out_edges.get(v).is_some_and(|m| m.contains_key(w))
    }

    /// Remove the edge, keeping every remaining edge and neighbour in insertion order.
    pub(crate) fn remove_edge(&mut self, e: &Edge) {
        let key = self.edge_key(&e.v, &e.w, e.name.as_deref());
        let is_edge_absent = self.edges.shift_remove(&key).is_none();
        if is_edge_absent {
            return;
        }
        Self::unlink(&mut self.out_edges, &e.v, &e.w, &key);
        Self::unlink(&mut self.in_edges, &e.w, &e.v, &key);
        if !self.is_directed {
            Self::unlink(&mut self.out_edges, &e.w, &e.v, &key);
            Self::unlink(&mut self.in_edges, &e.v, &e.w, &key);
        }
    }

    pub(crate) fn edges(&self) -> Vec<Edge> {
        self.edges.values().map(EdgeEntry::to_edge).collect()
    }

    pub(crate) fn in_edges(&self, v: &str) -> Vec<Edge> {
        self.adjacent_edges(&self.in_edges, v)
    }

    pub(crate) fn out_edges(&self, v: &str) -> Vec<Edge> {
        self.adjacent_edges(&self.out_edges, v)
    }

    /// Collect the edges into `v`, then the edges out of it.
    pub(crate) fn node_edges(&self, v: &str) -> Vec<Edge> {
        let mut edges = self.in_edges(v);
        let out_edges = self.out_edges(v);
        edges.extend(out_edges);
        edges
    }

    pub(crate) fn predecessors(&self, v: &str) -> Vec<String> {
        Self::neighbours_in(&self.in_edges, v)
    }

    pub(crate) fn successors(&self, v: &str) -> Vec<String> {
        Self::neighbours_in(&self.out_edges, v)
    }

    /// Collect the predecessors of `v`, then its successors not already collected.
    pub(crate) fn neighbors(&self, v: &str) -> Vec<String> {
        let mut neighbors = self.predecessors(v);
        for successor in self.successors(v) {
            let is_new_neighbor = !neighbors.contains(&successor);
            if is_new_neighbor {
                neighbors.push(successor);
            }
        }
        neighbors
    }

    fn edge_key(&self, v: &str, w: &str, name: Option<&str>) -> String {
        let is_reversed_undirected_pair = !self.is_directed && v > w;
        let (v, w) = if is_reversed_undirected_pair {
            (w, v)
        } else {
            (v, w)
        };
        match name {
            Some(name) => format!("{v}\x00{w}\x00{name}"),
            None => format!("{v}\x00{w}\x00\x01"),
        }
    }

    fn adjacent_edges(
        &self,
        adjacency: &HashMap<String, IndexMap<String, Vec<String>>>,
        v: &str,
    ) -> Vec<Edge> {
        let Some(neighbour_keys) = adjacency.get(v) else {
            return Vec::new();
        };
        neighbour_keys
            .values()
            .flatten()
            .filter_map(|key| self.edges.get(key))
            .map(EdgeEntry::to_edge)
            .collect()
    }

    fn neighbours_in(
        adjacency: &HashMap<String, IndexMap<String, Vec<String>>>,
        v: &str,
    ) -> Vec<String> {
        adjacency
            .get(v)
            .map(|neighbour_keys| neighbour_keys.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn link(
        adjacency: &mut HashMap<String, IndexMap<String, Vec<String>>>,
        from: &str,
        to: &str,
        key: &str,
    ) {
        adjacency
            .entry(from.to_string())
            .or_default()
            .entry(to.to_string())
            .or_default()
            .push(key.to_string());
    }

    fn unlink(
        adjacency: &mut HashMap<String, IndexMap<String, Vec<String>>>,
        from: &str,
        to: &str,
        key: &str,
    ) {
        let Some(neighbour_keys) = adjacency.get_mut(from) else {
            return;
        };
        let Some(keys) = neighbour_keys.get_mut(to) else {
            return;
        };
        keys.retain(|existing| existing != key);
        if keys.is_empty() {
            neighbour_keys.shift_remove(to);
        }
    }
}

impl EdgeEntry {
    fn to_edge(&self) -> Edge {
        Edge {
            v: self.v.clone(),
            w: self.w.clone(),
            name: self.name.clone(),
        }
    }
}

/// List the nodes reachable from `roots` through undirected neighbours, each after its unvisited neighbours.
pub(crate) fn postorder(graph: &Graph, roots: Vec<String>) -> Vec<String> {
    fn dfs(graph: &Graph, v: &str, visited: &mut HashSet<String>, result: &mut Vec<String>) {
        let is_first_visit = visited.insert(v.to_string());
        if !is_first_visit {
            return;
        }
        for w in graph.neighbors(v) {
            dfs(graph, &w, visited, result);
        }
        result.push(v.to_string());
    }

    let mut result = Vec::new();
    let mut visited = HashSet::new();
    for root in roots {
        dfs(graph, &root, &mut visited, &mut result);
    }
    result
}

/// List the nodes reachable from `roots` through undirected neighbours, each before its unvisited neighbours.
pub(crate) fn preorder(graph: &Graph, roots: Vec<String>) -> Vec<String> {
    fn dfs(graph: &Graph, v: &str, visited: &mut HashSet<String>, result: &mut Vec<String>) {
        let is_first_visit = visited.insert(v.to_string());
        if !is_first_visit {
            return;
        }
        result.push(v.to_string());
        for w in graph.neighbors(v) {
            dfs(graph, &w, visited, result);
        }
    }

    let mut result = Vec::new();
    let mut visited = HashSet::new();
    for root in roots {
        dfs(graph, &root, &mut visited, &mut result);
    }
    result
}
