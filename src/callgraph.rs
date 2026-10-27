use std::collections::{HashMap, HashSet};

#[derive(Debug)]
struct CallGraph {
    graph: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    fn new() -> Self {
        CallGraph {
            graph: HashMap::new(),
        }
    }
    // Do we care about adding without a callee?
    fn add_call(&mut self, caller: &str, callee: &str) {
        // We can add callee as the key to denote the terminating
        // end since callee can have many edges (callers)
        let g = self.graph.entry(callee.into()).or_default();
        g.insert(caller.into());
    }
    fn get_callers(&self, callee: &str) -> Vec<String> {
        self.graph
            .get(callee)
            .map(|g| g.into_iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use crate::callgraph::CallGraph;

    #[test]
    fn test_callee_insertion() {
        let mut c = CallGraph::new();
        c.add_call("Admin.retry", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Service.save", "Repository.insert");

        let callers = c.graph.get("Service.save").unwrap();
        assert_eq!(callers.len(), 2);
        assert!(callers.contains("Controller.create"));
        assert!(callers.contains("Admin.retry"));

        let callers = c.graph.get("Repository.insert").unwrap();
        assert_eq!(callers.len(), 1);
        assert!(callers.contains("Service.save"));
    }

    #[test]
    fn test_caller_retrieval() {
        let mut c = CallGraph::new();
        c.add_call("Admin.retry", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Service.save", "Repository.insert");
        let callers = c.get_callers("Service.save");
        assert_eq!(callers.len(), 2)
    }

    #[test]
    fn test_caller_retrieval_empty() {
        let c = CallGraph::new();
        let callers = c.get_callers("empty");
        assert!(callers.is_empty())
    }
}
