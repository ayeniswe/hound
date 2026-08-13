mod graph;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::graph::{
    Direction, FindQuery, Graph, Query, QueryEngine, RelationshipKind, RelationshipQuery,
    SymbolKindQuery,
};

fn collect_source_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            files.extend(collect_source_files(&path)?);
        } else if path.is_file()
            && ["cpp", "java", "py", "rs"].contains(
                &path
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            )
        {
            files.push(path);
        }
    }

    Ok(files)
}

fn main() {
    // Step 1. Build out parsing engine to ingest data
    let mut files = Vec::new();
    files.append(&mut collect_source_files(Path::new("data/TMCI")).unwrap());
    files.append(&mut collect_source_files(Path::new("data/TMDbAPI")).unwrap());
    let g = Graph::create(files).unwrap();

    // Step 2. Prototpe 0 - UQL
    let engine = QueryEngine::new(&g);

    let result = engine.execute(Query::Find(FindQuery {
        name: Some("getByTMDbValue".into()),
        kind: Some(SymbolKindQuery::Call),
        relationship: Some(RelationshipQuery {
            direction: Direction::Incoming,
            kind: Some(RelationshipKind::Calls),
            depth: 1,
        }),
    }));

    println!("RESULTS: {:?}", result);
}
// 2. Define universal domain model
//         ↓
// 3. Define Query IR
//         ↓
// 4. Build Graph query primitives
//         ↓
// 5. Build Query Executor
//         ↓
// 6. Build Query Planner
//         ↓
// 7. Build DSL lexer/parser
//         ↓
// 8. Build GUI → Query IR adapter
//         ↓
// 9. Build result/projection layer
//         ↓
// 10. Add optimization + indexes
