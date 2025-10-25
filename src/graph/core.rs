use std::{
    collections::HashMap,
    sync::{Arc, RwLock, Weak},
};

use anyhow::anyhow;

#[derive(Debug)]
pub struct Graph {
    root: Arc<Node>,

    pub(crate) nodes: RwLock<HashMap<String, Arc<Node>>>,
    // TODO: add bloomfilter back in when doing distributed
    // filter: RwLock<Bloom<String>>
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
    pub fn new() -> Graph {
        let root = Arc::new(Node::new("root"));
        let mut map = HashMap::new();
        map.insert(String::from("root"), root.clone());

        Graph {
            nodes: RwLock::new(map),
            root: root,
        }
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
            return Err(anyhow!("Edge already exists"));
        }

        children.push(Arc::downgrade(&child));
        Ok(())
    }

    fn get_or_create_node(&self, content: &str) -> anyhow::Result<Arc<Node>> {
        let mut nodes = self.nodes.write().unwrap();
        Ok(nodes
            .entry(content.to_owned())
            .or_insert_with(|| Arc::new(Node::new(content)))
            .clone())
    }
}
