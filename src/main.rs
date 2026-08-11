mod graph;

use std::path::Path;

use crate::graph::Graph;

fn main() {
    // Step 1. Build out parsing engine to ingest data
    let g = Graph::create(vec![
        Path::new("data/test.java"),
        Path::new("data/test.cpp"),
    ])
    .unwrap();
}
