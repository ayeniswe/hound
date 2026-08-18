mod graph;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::graph::{
    Direction, FindQuery, Graph, Query, QueryEngine, RelationshipKind, RelationshipQuery,
    SymbolKind, SymbolKindQuery,
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
    // files.append(&mut collect_source_files(Path::new("data/TMCI")).unwrap());
    // files.append(&mut collect_source_files(Path::new("data/TMDbAPI")).unwrap());
    files.append(&mut collect_source_files(Path::new("data/demo/java/static_import")).unwrap());
    // files.append(&mut collect_source_files(Path::new("data/demo/java/interface")).unwrap());
    // files.append(&mut collect_source_files(Path::new("data/demo/java/interface_default")).unwrap());
    // files.append(&mut collect_source_files(Path::new("data/demo/java/superclass")).unwrap());
    // files.append(&mut collect_source_files(Path::new("data/demo/java/current")).unwrap());
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

    for sym in g.symbols() {
        if matches!(sym.1.kind, SymbolKind::FunctionCall) {
            println!("SYMBOLS: {:?}", sym.1);
        }
    }

    // println!("RESULTS: {:?}", result);
}
