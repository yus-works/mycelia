#![cfg(test)]
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::task;

use crate::graph::core::{Graph, GraphEvent};

// Helper function to collect events from the channel
async fn collect_events(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<GraphEvent>,
    max_events: usize,
    timeout: Duration,
) -> Vec<GraphEvent> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;

    while events.len() < max_events {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(event)) => events.push(event),
            Ok(None) => break, // Channel closed
            Err(_) => break,   // Timeout
        }
    }

    events
}

#[tokio::test]
async fn test_basic_node_added_event() {
    let (graph, mut rx) = Graph::new();

    graph.add_edge("root", "child").unwrap();

    let events = collect_events(&mut rx, 2, Duration::from_millis(100)).await;

    assert_eq!(events.len(), 2);

    match &events[0] {
        GraphEvent::NodeAdded(name) => assert_eq!(name, "child"),
        _ => panic!("Expected NodeAdded event"),
    }

    match &events[1] {
        GraphEvent::EdgeAdded(parent, child) => {
            assert_eq!(parent, "root");
            assert_eq!(child, "child");
        }
        _ => panic!("Expected EdgeAdded event"),
    }
}

#[tokio::test]
async fn test_no_duplicate_node_events() {
    let (graph, mut rx) = Graph::new();

    // Add same edge twice - should only get one NodeAdded event
    graph.add_edge("root", "shared").unwrap();
    let _ = graph.add_edge("root", "shared"); // This should fail

    let events = collect_events(&mut rx, 10, Duration::from_millis(100)).await;

    let node_added_count = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::NodeAdded(_)))
        .count();

    // Should only have one NodeAdded for "shared"
    assert_eq!(
        node_added_count, 1,
        "Should only emit one NodeAdded event for shared node"
    );
}

#[tokio::test]
async fn test_events_match_graph_state() {
    let (graph, mut rx) = Graph::new();

    // Build a small graph
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("root", "B").unwrap();
    graph.add_edge("A", "C").unwrap();
    graph.add_edge("B", "C").unwrap(); // C already created

    let events = collect_events(&mut rx, 20, Duration::from_millis(100)).await;

    // Extract node names from NodeAdded events
    let nodes_from_events: HashSet<String> = events
        .iter()
        .filter_map(|e| match e {
            GraphEvent::NodeAdded(name) => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Should have A, B, C (root already exists)
    assert_eq!(nodes_from_events.len(), 3);
    assert!(nodes_from_events.contains("A"));
    assert!(nodes_from_events.contains("B"));
    assert!(nodes_from_events.contains("C"));

    // Extract edges from EdgeAdded events
    let edges_from_events: Vec<(String, String)> = events
        .iter()
        .filter_map(|e| match e {
            GraphEvent::EdgeAdded(parent, child) => {
                Some((parent.clone(), child.clone()))
            }
            _ => None,
        })
        .collect();

    assert_eq!(edges_from_events.len(), 4);

    // Verify graph state matches events
    assert_eq!(graph.node_count(), 4); // root + A + B + C
    assert_eq!(graph.get_root().get_children().len(), 2); // A and B
}

#[tokio::test]
async fn test_event_ordering() {
    let (graph, mut rx) = Graph::new();

    graph.add_edge("root", "new_node").unwrap();

    let events = collect_events(&mut rx, 2, Duration::from_millis(100)).await;

    // NodeAdded should come before EdgeAdded
    match &events[0] {
        GraphEvent::NodeAdded(_) => {}
        _ => panic!("First event should be NodeAdded"),
    }

    match &events[1] {
        GraphEvent::EdgeAdded(_, _) => {}
        _ => panic!("Second event should be EdgeAdded"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_event_generation() {
    let (graph, mut rx) = Graph::new();
    let graph = Arc::new(graph);

    let num_tasks = 10;
    let mut handles = vec![];

    for i in 0..num_tasks {
        let g = Arc::clone(&graph);
        let handle = task::spawn(async move {
            for j in 0..10 {
                let node = format!("node_{}_{}", i, j);
                let _ = g.add_edge("root", &node);
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for h in handles {
        h.await.unwrap();
    }

    // Collect all events
    let events =
        collect_events(&mut rx, 200, Duration::from_millis(500)).await;

    // Should have 100 NodeAdded + 100 EdgeAdded events
    assert_eq!(events.len(), 200);

    let node_added_count = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::NodeAdded(_)))
        .count();
    let edge_added_count = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::EdgeAdded(_, _)))
        .count();

    assert_eq!(node_added_count, 100);
    assert_eq!(edge_added_count, 100);
}

#[tokio::test]
async fn test_no_events_on_failed_edge() {
    let (graph, mut rx) = Graph::new();

    // Add edge successfully
    graph.add_edge("root", "child").unwrap();

    // Try to add duplicate (should fail)
    let result = graph.add_edge("root", "child");
    assert!(result.is_err());

    let events = collect_events(&mut rx, 10, Duration::from_millis(100)).await;

    // Should only have events from the first successful add_edge
    assert_eq!(events.len(), 2); // NodeAdded + EdgeAdded
}

#[tokio::test]
async fn test_events_for_self_loop() {
    let (graph, mut rx) = Graph::new();

    graph.add_edge("root", "root").unwrap();

    let events = collect_events(&mut rx, 10, Duration::from_millis(100)).await;

    // Should only have EdgeAdded (root already exists)
    assert_eq!(events.len(), 1);

    match &events[0] {
        GraphEvent::EdgeAdded(parent, child) => {
            assert_eq!(parent, "root");
            assert_eq!(child, "root");
        }
        _ => panic!("Expected EdgeAdded event for self-loop"),
    }
}

#[tokio::test]
async fn test_events_for_dag_with_shared_node() {
    let (graph, mut rx) = Graph::new();

    // Create DAG: root -> A,B  and  A,B -> shared
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("root", "B").unwrap();
    graph.add_edge("A", "shared").unwrap();
    graph.add_edge("B", "shared").unwrap(); // shared already exists

    let events = collect_events(&mut rx, 20, Duration::from_millis(100)).await;

    // Count NodeAdded for "shared"
    let shared_node_events = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::NodeAdded(name) if name == "shared"))
        .count();

    assert_eq!(
        shared_node_events, 1,
        "Should only emit one NodeAdded for shared node"
    );

    // Should have both edges to "shared"
    let edges_to_shared = events
        .iter()
        .filter(|e| {
            matches!(e, GraphEvent::EdgeAdded(_, child) if child == "shared")
        })
        .count();

    assert_eq!(edges_to_shared, 2, "Should have two edges to shared node");
}

#[tokio::test]
async fn test_high_throughput_events() {
    let (graph, mut rx) = Graph::new();

    let num_nodes = 1000;

    // Add many edges rapidly
    for i in 0..num_nodes {
        let node = format!("node_{}", i);
        graph.add_edge("root", &node).unwrap();
    }

    let events = collect_events(&mut rx, num_nodes * 2, Duration::from_secs(1))
        .await;

    assert_eq!(
        events.len(),
        num_nodes * 2,
        "Should receive all events even under high load"
    );
}

#[tokio::test]
async fn test_event_contents_accuracy() {
    let (graph, mut rx) = Graph::new();

    graph.add_edge("parent", "child").unwrap();

    let events = collect_events(&mut rx, 3, Duration::from_millis(100)).await;

    // Verify exact content of events
    assert_eq!(events.len(), 3);

    match &events[0] {
        GraphEvent::NodeAdded(name) => assert_eq!(name, "parent"),
        _ => panic!("Expected NodeAdded(parent)"),
    }

    match &events[1] {
        GraphEvent::NodeAdded(name) => assert_eq!(name, "child"),
        _ => panic!("Expected NodeAdded(child)"),
    }

    match &events[2] {
        GraphEvent::EdgeAdded(parent, child) => {
            assert_eq!(parent, "parent");
            assert_eq!(child, "child");
        }
        _ => panic!("Expected EdgeAdded(parent, child)"),
    }
}

#[tokio::test]
async fn test_events_with_special_characters() {
    let (graph, mut rx) = Graph::new();

    let special_name = "node/with\\special$chars";
    graph.add_edge("root", special_name).unwrap();

    let events = collect_events(&mut rx, 2, Duration::from_millis(100)).await;

    match &events[0] {
        GraphEvent::NodeAdded(name) => assert_eq!(name, special_name),
        _ => panic!("Expected NodeAdded with special characters"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_no_event_loss_under_contention() {
    let (graph, mut rx) = Graph::new();
    let graph = Arc::new(graph);

    let num_tasks = 50;
    let mut handles = vec![];

    for i in 0..num_tasks {
        let g = Arc::clone(&graph);

        let handle = task::spawn(async move {
            let node = format!("node_{}", i);
            g.add_edge("root", &node).unwrap();
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    // With unbounded channel, we should never lose events
    let events = collect_events(&mut rx, 100, Duration::from_millis(500)).await;

    assert_eq!(
        events.len(),
        100,
        "Should receive all 100 events (50 NodeAdded + 50 EdgeAdded)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_rapid_edge_creation_events() {
    let (graph, mut rx) = Graph::new();
    let graph = Arc::new(graph);

    let mut handles = vec![];

    // Rapidly create a tree structure
    for i in 0..20 {
        let g = Arc::clone(&graph);
        let handle = task::spawn(async move {
            let parent = format!("level1_{}", i);
            g.add_edge("root", &parent).unwrap();

            for j in 0..5 {
                let child = format!("level2_{}_{}", i, j);
                g.add_edge(&parent, &child).unwrap();
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    // Should have: 20 level1 nodes + 100 level2 nodes = 120 NodeAdded
    //              20 root->level1 + 100 level1->level2 = 120 EdgeAdded
    let events = collect_events(&mut rx, 240, Duration::from_secs(1)).await;

    assert_eq!(events.len(), 240);
}

#[tokio::test]
async fn test_event_order_consistency() {
    let (graph, mut rx) = Graph::new();

    // Create a chain: root -> A -> B -> C
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();
    graph.add_edge("B", "C").unwrap();

    let events = collect_events(&mut rx, 6, Duration::from_millis(100)).await;

    // Events should be: NodeAdded(A), EdgeAdded(root,A),
    //                    NodeAdded(B), EdgeAdded(A,B),
    //                    NodeAdded(C), EdgeAdded(B,C)

    assert!(matches!(&events[0], GraphEvent::NodeAdded(n) if n == "A"));
    assert!(matches!(&events[1], GraphEvent::EdgeAdded(p, c) if p == "root" && c == "A"));
    assert!(matches!(&events[2], GraphEvent::NodeAdded(n) if n == "B"));
    assert!(matches!(&events[3], GraphEvent::EdgeAdded(p, c) if p == "A" && c == "B"));
    assert!(matches!(&events[4], GraphEvent::NodeAdded(n) if n == "C"));
    assert!(matches!(&events[5], GraphEvent::EdgeAdded(p, c) if p == "B" && c == "C"));
}
