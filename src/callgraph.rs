use std::collections::{HashMap, HashSet};

#[derive(Debug)]
struct CallGraph {
    callees: HashMap<String, HashSet<String>>,
    callers: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    fn new() -> Self {
        CallGraph {
            callees: HashMap::new(),
            callers: HashMap::new(),
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
    fn remove_method(&mut self, method: &str) {
        if let Some(edges) = self.callers.remove(method) {
            for edge in edges {
                if let Some(neighbors) = self.callees.get_mut(&edge) {
                    neighbors.remove(method);
                }
            }
        }
        if let Some(edges) = self.callees.remove(method) {
            for edge in edges {
                if let Some(neighbors) = self.callers.get_mut(&edge) {
                    neighbors.remove(method);
                }
            }
        }
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
}

#[cfg(test)]
mod test {
    use crate::callgraph::CallGraph;

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
