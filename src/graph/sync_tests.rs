#![cfg(test)]
use std::sync::Arc;

use crate::graph::core::{Graph, Node};

#[test]
fn test_create_graph() {
    let graph = Graph::new();

    assert_eq!(graph.get_root().get_data(), "root");
    assert_eq!(graph.nodes.read().unwrap().len(), 1);
}

#[test]
fn test_root_has_no_children_initially() {
    let graph = Graph::new();
    assert_eq!(graph.get_root().get_children().len(), 0);
}

#[test]
fn test_add_edge_single_child() {
    let graph = Graph::new();

    graph.add_edge("root", "child").unwrap();

    assert_eq!(graph.node_count(), 2);

    let children = graph.get_node("root").unwrap().get_children();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].get_data(), "child");
}

#[test]
fn test_add_edge_multiple_children() {
    let graph = Graph::new();

    graph.add_edge("root", "child1").unwrap();
    graph.add_edge("root", "child2").unwrap();
    graph.add_edge("root", "child3").unwrap();

    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.get_node("root").unwrap().get_children().len(), 3);
}

#[test]
fn test_empty_string_as_data() {
    let graph = Graph::new();
    assert!(graph.add_edge("root", "").is_ok());
    assert!(graph.contains(""));
}

#[test]
fn test_special_characters_in_data() {
    let graph = Graph::new();
    assert!(graph.add_edge("root", "node/with\\special$chars").is_ok());
}

#[test]
fn test_disconnected_nodes() {
    let graph = Graph::new();
    graph.add_edge("A", "B").unwrap(); // not connected to root
    assert_eq!(graph.node_count(), 3); // root, A, B
    assert!(graph.get_node("A").is_some());
}

#[test]
fn test_nested_graph() {
    let graph = Graph::new();

    graph.add_edge("root", "child").unwrap();
    graph.add_edge("child", "grandchild").unwrap();

    assert_eq!(graph.node_count(), 3);

    let root = graph.get_node("root").unwrap();
    let child = graph.get_node("child").unwrap();

    assert_eq!(root.get_children().len(), 1);
    assert_eq!(child.get_children().len(), 1);
}

#[test]
fn test_graph_traversal() {
    let graph = Graph::new();

    graph.add_edge("root", "child1").unwrap();
    graph.add_edge("root", "child2").unwrap();
    graph.add_edge("child1", "grandchild").unwrap();

    // traverse and collect node data
    fn traverse(node: &Arc<Node>, visited: &mut Vec<String>) {
        visited.push(node.get_data().to_owned());
        let children = node.get_children();
        for child in children.iter() {
            traverse(child, visited);
        }
    }

    let mut visited = vec![];
    traverse(&graph.get_root(), &mut visited);

    assert_eq!(visited.len(), 4);
    assert!(visited.contains(&"root".to_string()));
    assert!(visited.contains(&"child1".to_string()));
    assert!(visited.contains(&"grandchild".to_string()));
}

#[test]
fn test_same_child_multiple_parents_allowed() {
    let graph = Graph::new();

    // add both parents to graph
    graph.add_edge("root", "parent1").unwrap();
    graph.add_edge("root", "parent2").unwrap();

    // add same child to first parent
    graph.add_edge("parent1", "shared_child").unwrap();

    // add same child to second parent
    let result = graph.add_edge("parent2", "shared_child");
    assert!(
        result.is_ok(),
        "Should allow same child to have multiple parents"
    );

    let p1 = graph.get_node("parent1").unwrap();
    let p2 = graph.get_node("parent2").unwrap();
    let shared_child = graph.get_node("shared_child").unwrap();

    // child appears in both parents' children
    assert_eq!(p1.get_children().len(), 1);
    assert_eq!(p2.get_children().len(), 1);
    assert!(Arc::ptr_eq(&p1.get_children()[0], &shared_child));
    assert!(Arc::ptr_eq(&p2.get_children()[0], &shared_child));
}

#[test]
fn test_duplicate_children_rejected() {
    let graph = Graph::new();

    let res1 = graph.add_edge("root", "child");
    let res2 = graph.add_edge("root", "child");

    assert!(res1.is_ok(), "Should succeed in adding first edge");
    assert!(res2.is_err(), "Should reject duplicate edge");

    assert!(
        res2.unwrap_err()
            .to_string()
            .contains("Edge already exists")
    );

    let root = graph.get_node("root").expect("Root should exist in graph");
    assert_eq!(root.get_children().len(), 1);
}

#[test]
fn test_get_node() {
    let graph = Graph::new();

    assert!(graph.add_edge("root", "A").is_ok());
    assert!(graph.add_edge("root", "B").is_ok());
    assert!(graph.add_edge("B", "C").is_ok());

    assert!(graph.get_node("A").is_some(), "Should find node");
    assert!(graph.get_node("B").is_some(), "Should find node");
    assert!(graph.get_node("C").is_some(), "Should find node");
    assert!(graph.get_node("root").is_some(), "Should find node");
}

#[test]
fn test_cycle_doesnt_leak_with_weak_refs() {
    let graph = Graph::new();
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();
    graph.add_edge("B", "A").unwrap(); // cycle

    // only the HashMap owns these nodes (+ get_node temporary reference)
    assert_eq!(Arc::strong_count(&graph.get_node("A").unwrap()), 2);
    assert_eq!(Arc::strong_count(&graph.get_node("B").unwrap()), 2);
}

// NOTE: there won't be any removal of nodes in a wiki crawler
// but this verifies logic i guess
#[test]
fn test_dead_weak_refs_filtered_out() {
    let graph = Graph::new();
    graph.add_edge("root", "child").unwrap();

    let child_arc = graph.get_node("child").unwrap();
    graph.add_edge("child", "grandchild").unwrap();

    // remove grandchild from hashmap (simulating node removal)
    graph.nodes.write().unwrap().remove("grandchild");

    // get_children should filter out dead weak ref
    let children = child_arc.get_children();
    assert_eq!(children.len(), 0, "Dead weak refs should be filtered");
}

#[test]
fn test_get_children_creates_temporary_ownership() {
    let graph = Graph::new();
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();

    let b = graph.get_node("B").unwrap();
    assert_eq!(Arc::strong_count(&b), 2); // HashMap + local binding
    drop(b);
    // now only HashMap owns B

    let a = graph.get_node("A").unwrap();
    {
        let children = a.get_children(); // upgrade() creates Arc<B>
        let b_ref = &children[0]; // simple borrow that doesnt increment
        assert_eq!(Arc::strong_count(b_ref), 2); // HashMap + children vec
    } // children dropped here
    // B's strong count back to 1 (only HashMap)

    // confirm B's strong count is at 1 without incrementing it
    let guard = graph.nodes.read().unwrap();
    let map_b_ref = guard.get("B").unwrap();

    assert_eq!(Arc::strong_count(map_b_ref), 1);
}

#[test]
fn test_self_loop_allowed() {
    let graph = Graph::new();

    assert!(
        graph.add_edge("root", "root").is_ok(),
        "Should succeed, pages can link to themselves"
    );

    let root = graph.get_node("root").unwrap();
    let children = root.get_children();

    assert_eq!(children.len(), 1);
    assert!(Arc::ptr_eq(&children[0], &root)); // child is itself!
}

#[test]
fn test_traversal_visits_each_node_once() {
    let graph = Graph::new();
    graph.add_edge("root", "left").unwrap();
    graph.add_edge("root", "right").unwrap();
    graph.add_edge("left", "shared").unwrap();
    graph.add_edge("right", "shared").unwrap();

    // proper dfs with visited tracking
    fn count_unique_visits(
        node: &Arc<Node>,

        // keep track of visited nodes
        visited: &mut std::collections::HashSet<String>,
    ) -> usize {
        // if not newly inserted return 0
        if !visited.insert(node.get_data().to_string()) {
            return 0; // Already visited
        }

        // count this node as 1
        let mut count = 1;

        // recursively visit children
        for child in node.get_children() {
            count += count_unique_visits(&child, visited);
        }

        // return 1 (self) + count of visited (unique) descendants
        count
    }

    let mut visited = std::collections::HashSet::new();
    let visits = count_unique_visits(&graph.get_root(), &mut visited);

    // should visit each node exactly once
    assert_eq!(visits, 4); // root + left + right + shared
}

#[test]
fn test_graph_query_methods() {
    let graph = Graph::new();
    graph.add_edge("root", "child").unwrap();

    assert!(graph.contains("root"));
    assert!(graph.contains("child"));
    assert!(!graph.contains("nonexistent"));

    assert!(graph.get_node("root").is_some());
    assert!(graph.get_node("child").is_some());
    assert!(graph.get_node("nonexistent").is_none());

    assert_eq!(graph.node_count(), 2);
}
