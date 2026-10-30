use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug)]
struct CallGraph {
    /// The methods that are called on by other mehtods
    ///
    /// `B <- called by A`
    callees: HashMap<String, HashSet<String>>,
    /// The methods that call other methods
    ///
    ///  `A -> calls B`
    callers: HashMap<String, HashSet<String>>,
    /// All methods contained in a file
    file_index: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    fn new() -> Self {
        CallGraph {
            callees: HashMap::new(),
            callers: HashMap::new(),
            file_index: HashMap::new(),
        }
    }
    fn add_call(&mut self, caller: &str, callee: &str) {
        // We keep bi-direcitonal lookup by indexing
        // both direction callee<->caller
        let g = self.callees.entry(callee.into()).or_default();
        g.insert(caller.into());

        let g = self.callers.entry(caller.into()).or_default();
        g.insert(callee.into());
    }
    fn remove_call(&mut self, caller: &str, callee: &str) {
        if let Some(callees) = self.callers.get_mut(caller) {
            callees.remove(callee);
        }
        if let Some(callers) = self.callees.get_mut(callee) {
            callers.remove(caller);
        }
    }
    fn remove_outgoing_edges(&mut self, method: &str) {
        if let Some(edges) = self.callers.remove(method) {
            for edge in edges {
                if let Some(neighbors) = self.callees.get_mut(&edge) {
                    neighbors.remove(method);
                }
            }
        }
    }
    fn remove_method(&mut self, method: &str) {
        self.file_index.remove(method);
        self.remove_outgoing_edges(method);
        // Remove incoming methods relationships
        if let Some(edges) = self.callees.remove(method) {
            for edge in edges {
                if let Some(neighbors) = self.callers.get_mut(&edge) {
                    neighbors.remove(method);
                }
            }
        }
        // The call graph but doesn't remove the method from file_index.
    }
    fn get_callers(&self, callee: &str) -> Vec<String> {
        self.callees
            .get(callee)
            .map(|g| g.into_iter().cloned().collect())
            .unwrap_or_default()
    }
    fn get_callees(&self, caller: &str) -> Vec<String> {
        self.callers
            .get(caller)
            .map(|g| g.into_iter().cloned().collect())
            .unwrap_or_default()
    }
    fn get_all_callers(&self, callee: &str) -> HashMap<String, usize> {
        let mut visited = HashMap::from([(callee.to_string(), 0usize)]); // D, C1, B2, A3, E3
        let mut queue: VecDeque<&str> = VecDeque::from([callee]); // D, C, B, AE

        while !queue.is_empty() {
            if let Some(method) = queue.pop_front() {
                if let Some(callers) = self.callees.get(method) {
                    let next_depth = visited[method] + 1;
                    for caller in callers {
                        // Prevent cycles by not revisiting seen edges
                        if !visited.contains_key(caller) {
                            visited.insert(caller.clone(), next_depth);
                            queue.push_back(caller);
                        }
                    }
                }
            }
        }

        visited.remove(callee);
        visited
    }
    fn upsert_file_calls(&mut self, file: &str, calls: &[(&str, &str)]) {
        // Clear out outgoing calls because we dont
        // want to mess up any calls that rely on these
        // methods
        let mut methods = self.file_index.remove(file).unwrap_or_default();
        // Perf issues not O(len)
        for method in methods.drain() {
            self.remove_outgoing_edges(&method);
        }

        for (caller, callee) in calls {
            methods.insert(caller.to_string());
            self.add_call(caller, callee);
        }

        self.file_index.insert(file.into(), methods);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::callgraph::CallGraph;
    #[test]
    fn test_file_calls_upsert() {
        let mut graph = CallGraph::new();
        // Test 1. Update file calls
        graph.upsert_file_calls(
            "Service.java",
            &[
                ("Service.save", "Repository.insert"),
                ("Service.save", "Logger.info"),
            ],
        );
        graph.upsert_file_calls("Controller.java", &[("Controller.create", "Service.save")]);
        graph.upsert_file_calls("Service.java", &[("Service.save", "Repository.update")]);

        assert!(graph.get_callers("Repository.insert").is_empty());
        assert!(graph.get_callers("Logger.info").is_empty());
        assert_eq!(graph.get_callers("Repository.update"), vec!["Service.save"]);
        assert_eq!(graph.get_callers("Service.save"), vec!["Controller.create"]);

        // Test 2. Clear stale data
        graph.upsert_file_calls("Service.java", &[]);

        assert!(graph.get_callees("Service.save").is_empty());
        assert_eq!(graph.get_callers("Service.save"), vec!["Controller.create"]);
    }
    #[test]
    fn test_retrive_all_caller() {
        // Test 1. Graph with a cycle
        let mut graph = CallGraph::new();

        graph.add_call("A", "B");
        graph.add_call("B", "C");
        graph.add_call("C", "D");
        graph.add_call("E", "B");
        graph.add_call("D", "B");

        assert_eq!(
            graph.get_all_callers("D"),
            HashMap::from([
                ("A".into(), 3),
                ("C".into(), 1),
                ("B".into(), 2),
                ("E".into(), 3)
            ])
        );

        // Test 2. Multi path enqueue
        let mut graph = CallGraph::new();

        graph.add_call("A", "B");
        graph.add_call("A", "C");
        graph.add_call("B", "D");
        graph.add_call("C", "D");

        assert_eq!(
            graph.get_all_callers("D"),
            HashMap::from([("A".into(), 2), ("B".into(), 1), ("C".into(), 1)])
        );

        // Test 3. Level depth
        let mut graph = CallGraph::new();

        graph.add_call("A", "B");
        graph.add_call("B", "D");
        graph.add_call("C", "D");
        graph.add_call("E", "C");

        assert_eq!(
            graph.get_all_callers("D"),
            HashMap::from([
                ("A".into(), 2),
                ("C".into(), 1),
                ("B".into(), 1),
                ("E".into(), 2)
            ])
        );
    }

    #[test]
    fn test_method_removal() {
        let mut c = CallGraph::new();
        c.add_call("A", "B");
        c.add_call("B", "C");
        c.add_call("C", "B");
        c.add_call("D", "B");

        c.remove_method("B");
        assert!(c.callees.get("B").is_none());
        assert!(c.callers.get("D").unwrap().is_empty());
        assert!(c.callers.get("A").unwrap().is_empty());
        assert!(c.callers.get("C").unwrap().is_empty());

        let mut c = CallGraph::new();
        c.add_call("A", "A");
        c.add_call("A", "B");
        c.add_call("B", "A");

        c.remove_method("A");
        assert!(c.callers.get("B").unwrap().is_empty());
        assert!(c.callers.get("A").is_none());

        let mut c = CallGraph::new();
        c.add_call("A", "C");
        c.add_call("A", "B");

        c.remove_method("D");
        assert!(c.callers.get("A").unwrap().get("C").is_some());
        assert!(c.callers.get("A").unwrap().get("B").is_some());
        assert!(c.callees.get("C").unwrap().get("A").is_some());
        assert!(c.callees.get("B").unwrap().get("A").is_some());
    }

    #[test]
    fn test_call_removal() {
        let mut c = CallGraph::new();
        c.add_call("Admin.retry", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Service.save", "Repository.insert");

        c.remove_call("Controller.create", "Service.save");
        assert!(
            c.callees
                .get("Service.save")
                .unwrap()
                .get("Controller.create")
                .is_none()
        );
        assert!(
            c.callers
                .get("Controller.create")
                .unwrap()
                .get("Service.save")
                .is_none()
        );
        assert!(
            c.callers
                .get("Admin.retry")
                .unwrap()
                .get("Service.save")
                .is_some()
        );
    }

    #[test]
    fn test_call_insertion() {
        let mut c = CallGraph::new();
        c.add_call("Admin.retry", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Service.save", "Repository.insert");

        // Test 1. all callees edges are created
        let callers = c.callees.get("Service.save").unwrap();
        assert_eq!(callers.len(), 2);
        assert!(callers.contains("Controller.create"));
        assert!(callers.contains("Admin.retry"));

        // Test 2. bidirectional edges callers<->callees created
        let callees = c.callers.get("Controller.create").unwrap();
        assert_eq!(callees.len(), 1);
        assert!(callees.contains("Service.save"));
        let callees = c.callers.get("Admin.retry").unwrap();
        assert_eq!(callees.len(), 1);
        assert!(callees.contains("Service.save"));
    }

    #[test]
    fn test_caller_retrieval() {
        let mut c = CallGraph::new();
        c.add_call("Admin.retry", "Service.save");
        c.add_call("Controller.create", "Service.save");
        c.add_call("Service.save", "Repository.insert");
        let callers = c.get_callers("Service.save");
        assert_eq!(callers.len(), 2);
        assert!(callers.contains(&"Admin.retry".to_string()));
        assert!(callers.contains(&"Controller.create".to_string()));
    }

    #[test]
    fn test_callee_retrieval() {
        let mut c = CallGraph::new();
        c.add_call("Controller.create", "Repository.insert");
        c.add_call("Controller.create", "Service.save");
        let callees = c.get_callees("Controller.create");
        assert_eq!(callees.len(), 2);
        assert!(callees.contains(&"Repository.insert".to_string()));
        assert!(callees.contains(&"Service.save".to_string()));
    }

    #[test]
    fn test_caller_retrieval_empty() {
        let c = CallGraph::new();
        let callers = c.get_callers("empty");
        assert!(callers.is_empty())
    }
}
