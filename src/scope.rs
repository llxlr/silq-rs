//! Scope management and symbol table for Silq.
//!
//! Scopes store variable bindings, handle nested scopes, and track
//! dependencies for termination analysis.

use crate::ast::{Declaration, Id, Interner};
use std::collections::HashMap;

/// A scope in the symbol table. Scopes form a nested hierarchy.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Parent scope (None for the global scope).
    pub parent: Option<Box<Scope>>,
    /// Bindings: name -> declaration.
    pub bindings: HashMap<usize, Declaration>,
    /// Dependency tracking for termination checking.
    pub dependencies: Vec<Id>,
    /// Unique ID suffix for generating fresh names.
    unique_counter: u64,
}

impl Scope {
    /// Create a new scope with an optional parent.
    pub fn new(parent: Option<Scope>) -> Self {
        Scope {
            parent: parent.map(Box::new),
            bindings: HashMap::new(),
            dependencies: vec![],
            unique_counter: 0,
        }
    }

    /// Create a new global (root) scope.
    pub fn global() -> Self {
        Scope::new(None)
    }

    /// Look up a name in this scope and its ancestors.
    pub fn lookup(&self, id: Id) -> Option<&Declaration> {
        if let Some(decl) = self.bindings.get(&id.0) {
            return Some(decl);
        }
        self.parent.as_ref().and_then(|p| p.lookup(id))
    }

    /// Look up a name only in this scope (not ancestors).
    pub fn lookup_local(&self, id: Id) -> Option<&Declaration> {
        self.bindings.get(&id.0)
    }

    /// Insert a binding into this scope.
    pub fn insert(&mut self, id: Id, decl: Declaration) {
        self.bindings.insert(id.0, decl);
    }

    /// Check if a name is bound in this scope or ancestors.
    pub fn contains(&self, id: Id) -> bool {
        self.lookup(id).is_some()
    }

    /// Check if a name is bound only in this scope.
    pub fn contains_local(&self, id: Id) -> bool {
        self.bindings.contains_key(&id.0)
    }

    /// Generate a unique name based on a base name.
    pub fn unique_name(&mut self, base: Id, interner: &mut Interner) -> Id {
        self.unique_counter += 1;
        let name = format!("{}_{}", interner.lookup(base), self.unique_counter);
        interner.intern(&name)
    }

    /// Create a child scope.
    pub fn child(&self) -> Self {
        Scope::new(Some(self.clone()))
    }

    /// Add a dependency (used for termination analysis).
    pub fn add_dependency(&mut self, id: Id) {
        if !self.dependencies.contains(&id) {
            self.dependencies.push(id);
        }
    }

    /// Get all bindings in this scope (does not include ancestors).
    pub fn local_bindings(&self) -> impl Iterator<Item = (&usize, &Declaration)> {
        self.bindings.iter()
    }

    /// Get all bindings visible from this scope (includes ancestors).
    pub fn all_bindings(&self) -> Vec<(Id, &Declaration)> {
        let mut result = Vec::new();
        let mut current = Some(self);
        while let Some(scope) = current {
            for (id, decl) in &scope.bindings {
                result.push((Id(*id), decl));
            }
            current = scope.parent.as_ref().map(|p| p.as_ref());
        }
        result
    }
}
