use std::collections::HashMap;

use crate::ast::CiprType;

pub struct TypeEnv {
    scopes: Vec<HashMap<String, CiprType>>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: &str, value_type: CiprType) {
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.to_string(), value_type);
    }

    pub fn get(&self, name: &str) -> Option<CiprType> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }
}
