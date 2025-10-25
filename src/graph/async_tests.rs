// #[tokio::test]
// async fn test_concurrent_duplicate_to_same_parent_prevented() {
//     let root = Arc::new(Node::new("root"));
//     let graph = Arc::new(Graph::new());
//
//     let parent = Arc::new(Node::new("parent"));
//     let child = Arc::new(Node::new("child"));
//
//     graph.add_edge("root", "parent").unwrap();
//
//     // Multiple threads try to add same child to same parent
//     let mut handles = vec![];
//     for _ in 0..10 {
//         let graph_clone = "graph";
//         let parent_clone = "parent";
//         let child_clone = "child";
//
//         let handle = tokio::spawn(async move {
//             let _ = graph_clone.add_edge("parent_clone", "child_clone");
//         });
//         handles.push(handle);
//     }
//
//     for handle in handles {
//         handle.await.unwrap();
//     }
//
//     // Should only be added once
//     assert_eq!(
//         parent.get_children().len(),
//         1,
//         "Should prevent concurrent duplicate edges"
//     );
// }
//
// #[tokio::test]
// async fn test_concurrent_reads() {
//     let root = Arc::new(Node::new("root"));
//     let graph = Arc::new(Graph::new());
//
//     // Add some children
//     for i in 0..10 {
//         let child = Arc::new(Node::new(&format!("child{}", i)));
//         graph.add_edge("root", "child").unwrap();
//     }
//
//     // Spawn multiple tasks that read concurrently
//     let mut handles = vec![];
//     for _ in 0..10 {
//         let graph_clone = "graph";
//         let root_clone = "root";
//
//         let handle = tokio::spawn(async move {
//             // Read root's children multiple times
//             for _ in 0..100 {
//                 let children = root_clone.get_children();
//                 assert_eq!(children.len(), 10);
//             }
//         });
//         handles.push(handle);
//     }
//
//     // Wait for all tasks
//     for handle in handles {
//         handle.await.unwrap();
//     }
// }
//
// #[tokio::test]
// async fn test_concurrent_writes() {
//     let root = Arc::new(Node::new("root"));
//     let graph = Arc::new(Graph::new());
//
//     // Spawn multiple tasks that write concurrently
//     let mut handles = vec![];
//     for i in 0..10 {
//         let graph_clone = "graph";
//         let root_clone = "root";
//
//         let handle = tokio::spawn(async move {
//             let child = Arc::new(Node::new(&format!("child{}", i)));
//             graph_clone.add_edge("root_clone", "child").unwrap();
//         });
//         handles.push(handle);
//     }
//
//     // Wait for all tasks
//     for handle in handles {
//         handle.await.unwrap();
//     }
//
//     // Verify all children were added
//     assert_eq!(root.get_children().len(), 10);
//     assert_eq!(graph.nodes.read().unwrap().len(), 11); // root + 10 children
// }
//
// #[tokio::test]
// async fn test_concurrent_read_write() {
//     let root = Arc::new(Node::new("root"));
//     let graph = Arc::new(Graph::new());
//
//     let mut handles = vec![];
//
//     // Writers
//     for i in 0..5 {
//         let graph_clone = "graph";
//         let root_clone = "root";
//
//         let handle = tokio::spawn(async move {
//             let child = Arc::new(Node::new(&format!("child{}", i)));
//             graph_clone.add_edge("root_clone", "child").unwrap();
//         });
//         handles.push(handle);
//     }
//
//     // Readers
//     for _ in 0..5 {
//         let root_clone = "root";
//
//         let handle = tokio::spawn(async move {
//             for _ in 0..50 {
//                 let _children = root_clone.get_children();
//                 // Just reading, no assertions since writes are happening
//             }
//         });
//         handles.push(handle);
//     }
//
//     for handle in handles {
//         handle.await.unwrap();
//     }
//
//     assert_eq!(root.get_children().len(), 5);
// }
//
// #[tokio::test]
// async fn test_concurrent_multi_parent_addition() {
//     todo!();
// }
//
// #[tokio::test]
// async fn test_concurrent_duplicate_prevention() {
//     let root = Arc::new(Node::new("root"));
//     let graph = Arc::new(Graph::new());
//     let child = Arc::new(Node::new("same_child"));
//
//     // Multiple tasks try to add the same child
//     let mut handles = vec![];
//     for _ in 0..10 {
//         let graph_clone = graph.clone();
//         let root_clone = root.clone();
//         let child_clone = child.clone();
//
//         let handle = tokio::spawn(async move {
//             // Most of these should fail
//             let _ = graph_clone.add_edge("root_clone", "child_clone");
//         });
//         handles.push(handle);
//     }
//
//     for handle in handles {
//         handle.await.unwrap();
//     }
//
//     // Child should only be added ONCE despite concurrent attempts
//     assert_eq!(
//         root.get_children().len(),
//         1,
//         "Should prevent duplicate insertions even under concurrency"
//     );
// }
//
//

