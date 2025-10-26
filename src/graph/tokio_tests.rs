#![cfg(test)]
use std::sync::Arc;
use std::time::Duration;
use tokio::task;

use crate::graph::core::Graph;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_massive_concurrent_edge_addition() {
    let graph = Arc::new(Graph::new());
    let num_tasks = 1000;
    let mut handles = vec![];

    for i in 0..num_tasks {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            let child = format!("child_{}", i);
            graph_clone.add_edge("root", &child)
        });
        handles.push(handle);
    }

    // join all tasks
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    assert_eq!(graph.node_count(), num_tasks + 1); // +1 for root
    assert_eq!(graph.get_root().get_children().len(), num_tasks);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_heavy_duplicate_contention() {
    let graph = Arc::new(Graph::new());
    let num_tasks = 5000;
    let mut handles = vec![];

    for _ in 0..num_tasks {
        let graph_clone = Arc::clone(&graph);
        let handle =
            task::spawn(async move { graph_clone.add_edge("root", "hotspot") });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await);
    }

    let successes = results
        .iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();

    assert_eq!(successes, 1, "Only one task should succeed");
    assert_eq!(graph.node_count(), 2); // root + hotspot
    assert_eq!(graph.get_root().get_children().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_large_scale_dag_construction() {
    let graph = Arc::new(Graph::new());
    let layers = 5;
    let nodes_per_layer = 50;
    let mut handles = vec![];

    // build DAG in layers
    for layer in 0..layers {
        for i in 0..nodes_per_layer {
            let graph_clone = Arc::clone(&graph);

            let handle = task::spawn(async move {
                let current = format!("L{}_N{}", layer, i);

                if layer == 0 {
                    // first layer connects to root
                    graph_clone.add_edge("root", &current).unwrap();
                } else {
                    // connect to nodes in previous layer
                    let prev =
                        format!("L{}_N{}", layer - 1, i % nodes_per_layer);
                    graph_clone.add_edge(&prev, &current).unwrap();
                }
            });
            handles.push(handle);
        }
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // should have root + (layers * nodes_per_layer) nodes
    let expected = 1 + (layers * nodes_per_layer);
    assert_eq!(graph.node_count(), expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_mixed_async_workload() {
    let graph = Arc::new(Graph::new());
    let num_writers = 100;
    let num_readers = 500;
    let mut handles = vec![];

    // spawn writers
    for i in 0..num_writers {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            for j in 0..10 {
                let node = format!("writer_{}_node_{}", i, j);
                let _ = graph_clone.add_edge("root", &node);
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    // spawn readers
    for _ in 0..num_readers {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            for _ in 0..20 {
                let _ = graph_clone.node_count();
                let _ = graph_clone.get_root().get_children();
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // verify all nodes were added
    let expected = 1 + (num_writers * 10);
    assert_eq!(graph.node_count(), expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_query_storm() {
    let graph = Arc::new(Graph::new());

    // setup: Add some nodes
    for i in 0..100 {
        let node = format!("node_{}", i);
        graph.add_edge("root", &node).unwrap();
    }

    let num_tasks = 2000;
    let mut handles = vec![];

    for i in 0..num_tasks {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            let node_name = format!("node_{}", i % 100);

            // rapid-fire queries
            for _ in 0..50 {
                assert!(graph_clone.contains(&node_name));
                assert!(graph_clone.get_node(&node_name).is_some());
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_traversals() {
    let graph = Arc::new(Graph::new());

    // build a tree
    for i in 0..10 {
        let parent = format!("level1_{}", i);
        graph.add_edge("root", &parent).unwrap();

        for j in 0..10 {
            let child = format!("level2_{}_{}", i, j);
            graph.add_edge(&parent, &child).unwrap();
        }
    }

    let num_traversers = 200;
    let mut handles = vec![];

    for _ in 0..num_traversers {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            let root = graph_clone.get_root();
            let level1 = root.get_children();

            let mut total_nodes = 1; // root
            total_nodes += level1.len();

            for l1_node in level1 {
                let level2 = l1_node.get_children();
                total_nodes += level2.len();
            }

            assert_eq!(total_nodes, 111); // 1 root + 10 level1 + 100 level2
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_operations_complete_in_time() {
    let graph = Arc::new(Graph::new());
    let num_tasks = 1000;

    let timeout = Duration::from_secs(5);

    let result = tokio::time::timeout(timeout, async {
        let mut handles = vec![];

        for i in 0..num_tasks {
            let graph_clone = Arc::clone(&graph);
            let handle = task::spawn(async move {
                let node = format!("node_{}", i);
                graph_clone.add_edge("root", &node).unwrap();

                // also do some reads
                let _ = graph_clone.node_count();
                let _ = graph_clone.get_node(&node);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }
    })
    .await;

    assert!(result.is_ok(), "Operations should complete within timeout");
    assert_eq!(graph.node_count(), num_tasks + 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_spawn_blocking_integration() {
    let graph = Arc::new(Graph::new());
    let num_tasks = 100;
    let mut handles = vec![];

    for i in 0..num_tasks {
        let graph_clone = Arc::clone(&graph);
        let handle = tokio::task::spawn_blocking(move || {
            // simulate CPU-intensive work with blocking operations
            for j in 0..10 {
                let node = format!("blocking_{}_{}", i, j);
                graph_clone.add_edge("root", &node).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(graph.node_count(), 1 + (num_tasks * 10));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_cancellation_safety() {
    let graph = Arc::new(Graph::new());
    let mut handles = vec![];

    // spawn many tasks, some of which we'll cancel
    for i in 0..100 {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            for j in 0..100 {
                let node = format!("node_{}_{}", i, j);
                if graph_clone.add_edge("root", &node).is_ok() {
                    tokio::task::yield_now().await;
                }
            }
        });
        handles.push(handle);
    }

    // let some tasks run
    tokio::time::sleep(Duration::from_millis(50)).await;

    // abort half the tasks
    for handle in handles.iter().step_by(2) {
        handle.abort();
    }

    // wait for remaining tasks
    for handle in handles.into_iter().step_by(2).skip(1) {
        let _ = handle.await;
    }

    // after cancellation, graph should still be in a valid state
    // note: We can't check exact counts because cancelled tasks may have
    // partially completed (added nodes but not edges, or vice versa)
    let node_count = graph.node_count();

    // just verify graph is still functional
    assert!(node_count >= 1, "Graph should at least have root");
    assert!(graph.contains("root"), "Root should exist");
    let _ = graph.get_root().get_children(); // should not panic
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_self_loop_tokio() {
    let graph = Arc::new(Graph::new());
    let num_tasks = 1000;
    let mut handles = vec![];

    for _ in 0..num_tasks {
        let graph_clone = Arc::clone(&graph);
        let handle =
            task::spawn(async move { graph_clone.add_edge("root", "root") });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await);
    }

    let successes = results
        .iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();

    assert_eq!(successes, 1, "Only one self-loop should succeed");
    assert_eq!(graph.node_count(), 1); // still just root
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_async_cycle_creation() {
    let graph = Arc::new(Graph::new());

    // setup initial chain
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();
    graph.add_edge("B", "C").unwrap();
    graph.add_edge("C", "D").unwrap();

    let mut handles = vec![];

    // multiple tasks try to create cycles
    for i in 0..50 {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            tokio::task::yield_now().await;

            match i % 3 {
                0 => graph_clone.add_edge("D", "A"),
                1 => graph_clone.add_edge("C", "B"),
                2 => graph_clone.add_edge("B", "A"),
                _ => unreachable!(),
            }
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await);
    }

    // at least one of each cycle type should succeed
    let d_a_success = results.iter().any(|r| {
        if let Ok(Ok(_)) = r {
            graph
                .get_node("D")
                .unwrap()
                .get_children()
                .iter()
                .any(|c| c.get_data() == "A")
        } else {
            false
        }
    });

    assert!(
        d_a_success
            || graph
                .get_node("D")
                .unwrap()
                .get_children()
                .iter()
                .any(|c| c.get_data() == "A")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_node_count_monotonic_async() {
    let graph = Arc::new(Graph::new());
    let mut handles = vec![];

    // writers
    for i in 0..50 {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            for j in 0..20 {
                let node = format!("node_{}_{}", i, j);
                let _ = graph_clone.add_edge("root", &node);
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    // readers checking monotonicity
    for _ in 0..100 {
        let graph_clone = Arc::clone(&graph);
        let handle = task::spawn(async move {
            let mut prev_count = 1;

            for _ in 0..100 {
                let current_count = graph_clone.node_count();
                assert!(
                    current_count >= prev_count,
                    "Count decreased: {} -> {}",
                    prev_count,
                    current_count
                );
                prev_count = current_count;
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
