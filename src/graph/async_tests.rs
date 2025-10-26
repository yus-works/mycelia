#![cfg(test)]
use std::sync::{Arc, Barrier};
use std::thread;

use crate::graph::core::Graph;

#[test]
fn test_concurrent_add_edges_no_conflicts() {
    let graph = Arc::new(Graph::new());
    let num_threads = 10;
    let mut handles = vec![];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            let child_name = format!("child_{}", i);
            graph_clone.add_edge("root", &child_name).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // all children should be added
    assert_eq!(graph.node_count(), num_threads + 1); // +1 for root

    let root = graph.get_root();
    assert_eq!(root.get_children().len(), num_threads);
}

#[test]
fn test_concurrent_add_same_edge_duplicate_detection() {
    let graph = Arc::new(Graph::new());
    let num_threads = 20;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // synchronize threads to maximize contention
            barrier_clone.wait();
            graph_clone.add_edge("root", "shared_child")
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for handle in handles {
        match handle.join().unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => {
                assert!(e.to_string().contains("Edge already exists"));
                error_count += 1;
            }
        }
    }

    // exactly one thread should succeed
    assert_eq!(
        success_count, 1,
        "Only one thread should successfully add the edge"
    );
    assert_eq!(
        error_count,
        num_threads - 1,
        "All other threads should fail"
    );

    // verify graph state
    assert_eq!(graph.node_count(), 2); // root + shared_child
    assert_eq!(graph.get_root().get_children().len(), 1);
}

#[test]
fn test_concurrent_read_write_operations() {
    let graph = Arc::new(Graph::new());
    let num_writers = 5;
    let num_readers = 10;
    let mut handles = vec![];

    // spawn writer threads
    for i in 0..num_writers {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let node_name = format!("writer_{}_node_{}", i, j);
                graph_clone.add_edge("root", &node_name).unwrap();
            }
        });
        handles.push(handle);
    }

    // spawn reader threads
    for i in 0..num_readers {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for _ in 0..20 {
                // perform various read operations
                let _ = graph_clone.node_count();
                let _ = graph_clone.contains("root");
                let _ = graph_clone.get_node("root");
                let _ = graph_clone.get_root().get_children();

                // try to read a node that might exist
                let node_name = format!("writer_{}_node_5", i % num_writers);
                let _ = graph_clone.contains(&node_name);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // verify final state
    assert_eq!(graph.node_count(), num_writers * 10 + 1); // +1 for root
}

#[test]
fn test_concurrent_nested_graph_construction() {
    let graph = Arc::new(Graph::new());
    let num_chains = 8;
    let chain_length = 10;
    let mut handles = vec![];

    for i in 0..num_chains {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            let mut prev = "root".to_string();
            for j in 0..chain_length {
                let current = format!("chain_{}_node_{}", i, j);
                graph_clone.add_edge(&prev, &current).unwrap();
                prev = current;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // each chain creates chain_length nodes
    assert_eq!(graph.node_count(), num_chains * chain_length + 1); // +1 for root

    // root should have num_chains direct children
    assert_eq!(graph.get_root().get_children().len(), num_chains);
}

#[test]
fn test_concurrent_dag_construction() {
    let graph = Arc::new(Graph::new());
    let barrier = Arc::new(Barrier::new(4));
    let mut handles = vec![];

    // thread 1: root -> A -> shared
    let graph1 = Arc::clone(&graph);
    let barrier1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier1.wait();
        graph1.add_edge("root", "A").unwrap();
        graph1.add_edge("A", "shared").unwrap();
    }));

    // thread 2: root -> B -> shared
    let graph2 = Arc::clone(&graph);
    let barrier2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier2.wait();
        graph2.add_edge("root", "B").unwrap();
        graph2.add_edge("B", "shared").unwrap();
    }));

    // thread 3: root -> C -> shared
    let graph3 = Arc::clone(&graph);
    let barrier3 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier3.wait();
        graph3.add_edge("root", "C").unwrap();
        graph3.add_edge("C", "shared").unwrap();
    }));

    // thread 4: Concurrent reader
    let graph4 = Arc::clone(&graph);
    let barrier4 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier4.wait();
        for _ in 0..50 {
            let _ = graph4.get_node("shared");
            let _ = graph4.contains("shared");
            thread::yield_now();
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // verify structure: root, A, B, C, shared = 5 nodes
    assert_eq!(graph.node_count(), 5);

    // verify shared node has multiple parents
    let shared = graph.get_node("shared").unwrap();
    assert_eq!(Arc::strong_count(&shared), 2); // hashMap + local binding

    // verify each parent has shared as child
    let a = graph.get_node("A").unwrap();
    let b = graph.get_node("B").unwrap();
    let c = graph.get_node("C").unwrap();

    assert_eq!(a.get_children().len(), 1);
    assert_eq!(b.get_children().len(), 1);
    assert_eq!(c.get_children().len(), 1);
}

#[test]
fn test_concurrent_traversal_during_modification() {
    let graph = Arc::new(Graph::new());
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // writer thread: continuously adds nodes
    let graph_writer = Arc::clone(&graph);
    let barrier_writer = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier_writer.wait();
        for i in 0..50 {
            let node_name = format!("node_{}", i);
            graph_writer.add_edge("root", &node_name).unwrap();
        }
    }));

    // traverser thread 1
    let graph_traverser1 = Arc::clone(&graph);
    let barrier_traverser1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier_traverser1.wait();
        for _ in 0..30 {
            let root = graph_traverser1.get_root();
            let children = root.get_children();
            // traverse children
            for child in children {
                let _ = child.get_data();
                let _ = child.get_children();
            }
            thread::yield_now();
        }
    }));

    // traverser thread 2
    let graph_traverser2 = Arc::clone(&graph);
    let barrier_traverser2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier_traverser2.wait();
        for _ in 0..30 {
            let root = graph_traverser2.get_root();
            let children = root.get_children();
            let count = children.len();
            // count should be consistent
            assert!(count <= 51); // root + up to 50 nodes
            thread::yield_now();
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(graph.node_count(), 51); // root + 50 nodes
}

#[test]
fn test_concurrent_stress_test() {
    let graph = Arc::new(Graph::new());
    let num_threads = 20;
    let operations_per_thread = 100;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for op in 0..operations_per_thread {
                match op % 4 {
                    0 => {
                        // add edge
                        let parent = format!("node_{}", thread_id);
                        let child = format!("node_{}_{}", thread_id, op);
                        let _ = graph_clone.add_edge(&parent, &child);
                    }
                    1 => {
                        // query existence
                        let node_name = format!("node_{}", thread_id % 10);
                        let _ = graph_clone.contains(&node_name);
                    }
                    2 => {
                        // get node
                        let node_name = format!("node_{}", thread_id);
                        let _ = graph_clone.get_node(&node_name);
                    }
                    3 => {
                        // traverse children
                        if let Some(node) = graph_clone.get_node("root") {
                            let _ = node.get_children();
                        }
                    }
                    _ => unreachable!(),
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // graph should be in a valid state
    let final_count = graph.node_count();
    assert!(final_count > 1, "Should have created multiple nodes");

    // verify root is still accessible
    assert_eq!(graph.get_root().get_data(), "root");
}

#[test]
fn test_concurrent_self_loop_creation() {
    let graph = Arc::new(Graph::new());
    let num_threads = 10;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();
            graph_clone.add_edge("root", "root")
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for handle in handles {
        match handle.join().unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    // only one self-loop should be added
    assert_eq!(success_count, 1);
    assert_eq!(error_count, num_threads - 1);
    assert_eq!(graph.node_count(), 1); // still just root
}

#[test]
fn test_concurrent_cycle_creation() {
    let graph = Arc::new(Graph::new());

    // setup: root -> A -> B
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = vec![];

    // thread 1: B -> A (creates cycle)
    let graph1 = Arc::clone(&graph);
    let barrier1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier1.wait();
        graph1.add_edge("B", "A").unwrap();
    }));

    // thread 2: A -> C (linear extension)
    let graph2 = Arc::clone(&graph);
    let barrier2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        barrier2.wait();
        graph2.add_edge("A", "C").unwrap();
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // both operations should succeed
    assert_eq!(graph.node_count(), 4); // root, A, B, C

    // verify cycle exists: A -> B -> A
    let a = graph.get_node("A").unwrap();
    let b = graph.get_node("B").unwrap();

    assert!(
        a.get_children()
            .iter()
            .any(|c| c.get_data() == "B" || c.get_data() == "C")
    );
    assert!(b.get_children().iter().any(|c| c.get_data() == "A"));
}

#[test]
fn test_concurrent_get_or_create_node() {
    let graph = Arc::new(Graph::new());
    let num_threads = 50;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    // all threads try to create edges from root to same set of nodes
    let target_nodes = vec!["shared1", "shared2", "shared3"];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);
        let targets = target_nodes.clone();

        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let target = targets[i % targets.len()];
            graph_clone.add_edge("root", target)
        });
        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        results.push(handle.join().unwrap());
    }

    // only 3 edges should succeed (one per unique target)
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, target_nodes.len());

    // verify only the target nodes were created
    assert_eq!(graph.node_count(), 1 + target_nodes.len()); // root + targets
    assert_eq!(graph.get_root().get_children().len(), target_nodes.len());
}

#[test]
fn test_no_deadlock_with_nested_locks() {
    let graph = Arc::new(Graph::new());
    let num_threads = 10;
    let mut handles = vec![];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for j in 0..20 {
                let parent = format!("parent_{}", i);
                let child = format!("child_{}_{}", i, j);

                // this acquires write lock on nodes, then write lock on parent.children
                let _ = graph_clone.add_edge(&parent, &child);

                // this acquires read lock on nodes
                let _ = graph_clone.contains(&child);

                // this acquires read lock on nodes, then read lock on children
                if let Some(node) = graph_clone.get_node(&parent) {
                    let _ = node.get_children();
                }
            }
        });
        handles.push(handle);
    }

    // if there's a deadlock, this will hang
    for handle in handles {
        handle.join().unwrap();
    }

    // test passes if we get here without hanging
    assert!(graph.node_count() > 1);
}

#[test]
fn test_concurrent_node_count_consistency() {
    let graph = Arc::new(Graph::new());
    let num_writers = 5;
    let num_readers = 10;
    let barrier = Arc::new(Barrier::new(num_writers + num_readers));
    let mut handles = vec![];

    // writers
    for i in 0..num_writers {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            for j in 0..10 {
                let node = format!("writer_{}_node_{}", i, j);
                let _ = graph_clone.add_edge("root", &node);
            }
        }));
    }

    // readers - track if counts are monotonically increasing
    for _ in 0..num_readers {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            let mut prev_count = 1; // at least root

            for _ in 0..50 {
                let current_count = graph_clone.node_count();

                // count should never decrease
                assert!(
                    current_count >= prev_count,
                    "Node count should be monotonically increasing: {} -> {}",
                    prev_count,
                    current_count
                );

                prev_count = current_count;
                thread::yield_now();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_contains_get_node_consistency() {
    let graph = Arc::new(Graph::new());
    let barrier = Arc::new(Barrier::new(2));

    // writer
    let graph_writer = Arc::clone(&graph);
    let barrier_writer = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        barrier_writer.wait();
        for i in 0..100 {
            let node = format!("node_{}", i);
            graph_writer.add_edge("root", &node).unwrap();
        }
    });

    // reader - if contains returns true, get_node should return Some
    let graph_reader = Arc::clone(&graph);
    let barrier_reader = Arc::clone(&barrier);
    let reader = thread::spawn(move || {
        barrier_reader.wait();
        for i in 0..100 {
            let node = format!("node_{}", i);

            if graph_reader.contains(&node) {
                // if contains says it exists, get_node must return Some
                assert!(
                    graph_reader.get_node(&node).is_some(),
                    "contains returned true but get_node returned None for {}",
                    node
                );
            }

            thread::yield_now();
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}
