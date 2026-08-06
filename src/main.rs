mod graph;

use std::path::Path;

use crate::graph::create_graph;

fn main() {
    // Step 1. Build out parsing engine to ingest data
    let data = Path::new("data/test.cpp");
    // let data = Path::new("data/test.java");
    let g = create_graph(vec![data]).unwrap();
    for sym in g.symbols() {
        println!("{:?}", sym)
    }
    for sym in g.relationships() {
        println!("{:?}", sym)
    }
}