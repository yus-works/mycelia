#![cfg(test)]
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use tracing::warn;

use crate::graph::core::Graph;

#[test]
fn test_aggressive_duplicate_edge_hammering() {
    let graph = Arc::new(Graph::new_without_events());
    let num_threads = 100;
    let mut handles = vec![];

    // no barrier - let threads race naturally
    for _ in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            // try multiple times to increase contention
            for _ in 0..10 {
                let _ = graph_clone.add_edge("root", "contested");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // should still only have one edge
    assert_eq!(
        graph.node_count(),
        2,
        "Should have exactly root + contested"
    );
    assert_eq!(
        graph.get_root().get_children().len(),
        1,
        "Root should have exactly one child"
    );
}

#[test]
fn test_interleaved_get_or_create() {
    let graph = Arc::new(Graph::new_without_events());
    let num_threads = 50;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // all threads try to create edges involving the same nodes
            // but in different parent-child relationships
            let _ = graph_clone.add_edge("A", "B");
            let _ = graph_clone.add_edge("B", "C");
            let _ = graph_clone.add_edge("C", "A"); // cycle

            // also try thread-specific edges
            let thread_node = format!("thread_{}", i);
            let _ = graph_clone.add_edge("A", &thread_node);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // should have: root, A, B, C, + num_threads thread-specific nodes
    let expected = 4 + num_threads;
    assert_eq!(graph.node_count(), expected);

    // verify edges
    assert!(
        graph
            .get_node("A")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "B")
    );
    assert!(
        graph
            .get_node("B")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "C")
    );
    assert!(
        graph
            .get_node("C")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "A")
    );
}

#[test]
fn test_rapid_add_and_read_same_parent() {
    let graph = Arc::new(Graph::new_without_events());
    let num_threads = 20;
    let mut handles = vec![];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                // writer threads
                for j in 0..50 {
                    let child = format!("child_{}_{}", i, j);
                    let _ = graph_clone.add_edge("root", &child);
                }
            } else {
                // reader threads - constantly check children
                for _ in 0..100 {
                    let root = graph_clone.get_root();
                    let children = root.get_children();
                    let count = children.len();

                    // verify we can actually read the data
                    for child in children {
                        let _ = child.get_data();
                    }

                    // count should be sane
                    assert!(
                        count <= 500,
                        "Children count should be reasonable"
                    );
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_complex_dag_concurrent_construction() {
    let graph = Arc::new(Graph::new_without_events());
    let num_threads = 10;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    // each thread creates a diamond pattern: top -> left/right -> bottom
    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let top = format!("top_{}", i);
            let left = format!("left_{}", i);
            let right = format!("right_{}", i);
            let bottom = format!("bottom_{}", i);

            graph_clone.add_edge("root", &top).unwrap();
            graph_clone.add_edge(&top, &left).unwrap();
            graph_clone.add_edge(&top, &right).unwrap();
            graph_clone.add_edge(&left, &bottom).unwrap();
            graph_clone.add_edge(&right, &bottom).unwrap(); // shared bottom
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // each thread creates 4 unique nodes (top, left, right, bottom)
    // total: root + (4 * num_threads)
    assert_eq!(graph.node_count(), 1 + (4 * num_threads));

    // verify structure of first diamond
    let bottom0 = graph.get_node("bottom_0").unwrap();
    // bottom should still be in the graph (not dropped)
    assert_eq!(Arc::strong_count(&bottom0), 2); // hashMap + local
}

#[test]
fn test_weak_reference_upgrade_under_contention() {
    let graph = Arc::new(Graph::new_without_events());

    // create a parent with many children
    for i in 0..100 {
        let child = format!("child_{}", i);
        graph.add_edge("root", &child).unwrap();
    }

    let num_threads = 20;
    let mut handles = vec![];

    for _ in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let root = graph_clone.get_root();
                let children = root.get_children();

                // all weak refs should successfully upgrade
                assert_eq!(
                    children.len(),
                    100,
                    "All children should be accessible"
                );

                // verify each child is valid
                for child in children {
                    assert!(child.get_data().starts_with("child_"));
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_modification_different_subtrees() {
    let graph = Arc::new(Graph::new_without_events());

    // setup: Create initial structure
    for i in 0..10 {
        let subtree_root = format!("subtree_{}", i);
        graph.add_edge("root", &subtree_root).unwrap();
    }

    let num_threads = 10;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for i in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // each thread modifies its own subtree
            let subtree_root = format!("subtree_{}", i);

            for j in 0..50 {
                let node = format!("subtree_{}_node_{}", i, j);
                graph_clone.add_edge(&subtree_root, &node).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // verify each subtree has correct structure
    for i in 0..num_threads {
        let subtree_root = format!("subtree_{}", i);
        let node = graph.get_node(&subtree_root).unwrap();
        assert_eq!(node.get_children().len(), 50);
    }
}

#[test]
fn test_simultaneous_cycle_creation() {
    let graph = Arc::new(Graph::new_without_events());

    // setup: A -> B -> C -> D
    graph.add_edge("root", "A").unwrap();
    graph.add_edge("A", "B").unwrap();
    graph.add_edge("B", "C").unwrap();
    graph.add_edge("C", "D").unwrap();

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = vec![];

    // thread 1: D -> A (big cycle)
    let g1 = Arc::clone(&graph);
    let b1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b1.wait();
        g1.add_edge("D", "A").unwrap();
    }));

    // thread 2: C -> A (medium cycle)
    let g2 = Arc::clone(&graph);
    let b2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b2.wait();
        g2.add_edge("C", "A").unwrap();
    }));

    // thread 3: B -> A (small cycle)
    let g3 = Arc::clone(&graph);
    let b3 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b3.wait();
        g3.add_edge("B", "A").unwrap();
    }));

    // thread 4: Traverser
    let g4 = Arc::clone(&graph);
    let b4 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b4.wait();
        for _ in 0..50 {
            let root = g4.get_root();
            let _ = root.get_children();
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // verify all cycles were created
    assert!(
        graph
            .get_node("D")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "A")
    );
    assert!(
        graph
            .get_node("C")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "A")
    );
    assert!(
        graph
            .get_node("B")
            .unwrap()
            .get_children()
            .iter()
            .any(|c| c.get_data() == "A")
    );
}

#[test]
fn test_no_phantom_duplicates() {
    let graph = Arc::new(Graph::new_without_events());
    let num_iterations = 100;

    for iteration in 0..num_iterations {
        let num_threads = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        let parent = format!("parent_{}", iteration);
        let child = format!("child_{}", iteration);

        for _ in 0..num_threads {
            let graph_clone = Arc::clone(&graph);
            let barrier_clone = Arc::clone(&barrier);
            let p = parent.clone();
            let c = child.clone();

            let handle = thread::spawn(move || {
                barrier_clone.wait();
                graph_clone.add_edge(&p, &c)
            });
            handles.push(handle);
        }

        let results: Vec<_> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        let successes =
            results.iter().filter(|r| matches!(r, Ok(true))).count();
        assert_eq!(
            successes, 1,
            "Iteration {}: Expected exactly 1 success, got {}",
            iteration, successes
        );

        // verify parent has exactly one child
        let parent_node = graph.get_node(&parent).unwrap();
        assert_eq!(
            parent_node.get_children().len(),
            1,
            "Parent should have exactly one child"
        );
    }
}

#[test]
fn test_timing_sensitive_operations() {
    let graph = Arc::new(Graph::new_without_events());
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // thread 1: Slow writer
    let g1 = Arc::clone(&graph);
    let b1 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b1.wait();
        for i in 0..20 {
            let node = format!("slow_{}", i);
            g1.add_edge("root", &node).unwrap();
            thread::sleep(Duration::from_micros(100));
        }
    }));

    // thread 2: Fast writer
    let g2 = Arc::clone(&graph);
    let b2 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b2.wait();
        for i in 0..100 {
            let node = format!("fast_{}", i);
            let _ = g2.add_edge("root", &node);
        }
    }));

    // thread 3: Fast reader
    let g3 = Arc::clone(&graph);
    let b3 = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
        b3.wait();
        for _ in 0..200 {
            // just verify reads don't panic - snapshot consistency
            // cannot be guaranteed without holding locks
            let _ = g3.node_count();
            let _ = g3.get_root().get_children().len();

            // these individual operations should always work
            let _ = g3.contains("root");
            let _ = g3.get_node("root");
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_reference_counting_integrity() {
    let graph = Arc::new(Graph::new_without_events());

    // add nodes
    for i in 0..50 {
        let node = format!("node_{}", i);
        graph.add_edge("root", &node).unwrap();
    }

    let num_threads = 10;
    let mut handles = vec![];

    for _ in 0..num_threads {
        let graph_clone = Arc::clone(&graph);
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let node_name = format!("node_{}", i);

                // get node multiple times
                let n1 = graph_clone.get_node(&node_name);
                let n2 = graph_clone.get_node(&node_name);

                if let (Some(node1), Some(node2)) = (n1, n2) {
                    // they should be the same Arc
                    assert!(Arc::ptr_eq(&node1, &node2));

                    // strong count should be reasonable
                    // hashMap(1) + n1(1) + n2(1) = 3 minimum
                    let count = Arc::strong_count(&node1);
                    assert!(
                        count >= 3 && count < 100,
                        "Unexpected strong count: {}",
                        count
                    );
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // after all threads complete, each node should have strong count of 2
    // (HashMap + temporary from get_node)
    for i in 0..50 {
        let node_name = format!("node_{}", i);
        let node = graph.get_node(&node_name).unwrap();
        assert_eq!(
            Arc::strong_count(&node),
            2,
            "Node {} has wrong strong count",
            node_name
        );
    }
}
