use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, RwLock, Weak},
};

use anyhow::anyhow;
use tokio::sync::mpsc;
use tracing::warn;

pub enum GraphEvent {
    NodeAdded(String),
    EdgeAdded(String, String),
}

// NOTE: Tokio's RwLock might be marginally better but idk

#[derive(Debug)]
pub struct Graph {
    root: Arc<Node>,

    pub(crate) nodes: RwLock<HashMap<String, Arc<Node>>>,
    // TODO: add bloomfilter back in when doing distributed
    // filter: RwLock<Bloom<String>>
    events_tx: Option<tokio::sync::mpsc::UnboundedSender<GraphEvent>>,
}

#[derive(Debug)]
pub struct Node {
    data: String,

    children: RwLock<Vec<Weak<Node>>>,
}

impl Node {
    pub fn new(data: &str) -> Node {
        Node {
            data: data.to_owned(),
            children: RwLock::new(vec![]),
        }
    }

    pub fn get_data(&self) -> &str {
        &self.data
    }

    pub fn get_children(&self) -> Vec<Arc<Node>> {
        self.children
            .read()
            .unwrap()
            .iter()
            .filter_map(|weak| weak.upgrade()) // filter rejects all dead refs
            .collect()
    }
}

impl Graph {
    pub fn new() -> (Graph, mpsc::UnboundedReceiver<GraphEvent>) {
        let root = Arc::new(Node::new("root"));
        let mut map = HashMap::new();
        map.insert(String::from("root"), root.clone());

        let (tx, rx) = mpsc::unbounded_channel();

        (
            Graph {
                nodes: RwLock::new(map),
                root: root,
                events_tx: Some(tx),
            },
            rx,
        )
    }

    pub fn get_root(&self) -> Arc<Node> {
        self.root.clone()
    }

    /// WARN: acquires nodes lock
    pub fn node_count(&self) -> usize {
        self.nodes.read().unwrap().len()
    }

    /// WARN: acquires nodes lock
    pub fn contains(&self, content: &str) -> bool {
        self.nodes.read().unwrap().contains_key(content)
    }

    /// WARN: acquires nodes lock
    pub fn get_node(&self, content: &str) -> Option<Arc<Node>> {
        self.nodes.read().unwrap().get(content).cloned()
    }

    // TODO: disjointed graphs allowed for now
    pub fn add_edge(
        &self,
        parent_content: &str,
        child_content: &str,
    ) -> anyhow::Result<()> {
        // get canonical nodes (creates if needed, returns existing if present)
        let parent = self.get_or_create_node(parent_content)?;
        let child = self.get_or_create_node(child_content)?;

        {
            // check duplicate edge using ptr_eq
            let mut children = parent.children.write().unwrap();

            if children.iter().any(|c| {
                match c.upgrade() {
                    // compare if node exists
                    Some(arc) => Arc::ptr_eq(&arc, &child),

                    // node doesn't exist anymore
                    None => false,
                }
            }) {
                warn!(
                    "Edge ({} -> {}) already exists",
                    parent_content, child_content
                );
                return Ok(());
            }

            children.push(Arc::downgrade(&child));
        } // scoped to drop lock before channel stuff

        if let Some(tx) = &self.events_tx {
            tx.send(GraphEvent::EdgeAdded(
                parent_content.to_owned(),
                child_content.to_owned(),
            ))
            .map_err(|e| anyhow!("Event dropped: {}", e))?;
        }

        Ok(())
    }

    fn get_or_create_node(&self, content: &str) -> anyhow::Result<Arc<Node>> {
        let (node, is_new) = {
            let mut nodes = self.nodes.write().unwrap();

            match nodes.entry(content.to_owned()) {
                Entry::Vacant(e) => {
                    let node = Arc::new(Node::new(content));
                    e.insert(node.clone());
                    (node, true)
                }
                Entry::Occupied(e) => (e.get().clone(), false),
            }
        };

        if is_new {
            if let Some(tx) = &self.events_tx {
                tx.send(GraphEvent::NodeAdded(content.to_owned())).map_err(
                    |e| anyhow!("Failed to send NodeAdded event: {}", e),
                )?;
            }
        }

        Ok(node)
    }
}

#[cfg(test)]
impl Graph {
    pub fn new_without_events() -> Graph {
        let root = Arc::new(Node::new("root"));
        let mut map = HashMap::new();
        map.insert(String::from("root"), root.clone());

        Graph {
            nodes: RwLock::new(map),
            root: root,
            events_tx: None,
        }
    }
}
