mod graph;

use std::path::Path;

use crate::graph::create_graph;

fn main() {
    // Step 1. Build out parsing engine to ingest data
    let g = create_graph(vec![
        Path::new("data/test.java"),
        Path::new("data/test.cpp"),
    ])
    .unwrap();
    for sym in g.symbols() {
        println!("{:?}", sym)
    }
    for sym in g.relationships() {
        println!("{:?}", sym)
    }
}
