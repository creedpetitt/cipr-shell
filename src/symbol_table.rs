use std::collections::HashMap;

use inkwell::values::PointerValue;

pub struct SymbolTable<'ctx> {
    scopes: Vec<HashMap<String, PointerValue<'ctx>>>,
}

impl<'ctx> SymbolTable<'ctx> {
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

    pub fn define(&mut self, name: &str, pointer: PointerValue<'ctx>) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), pointer);
    }

    pub fn get(&self, name: &str) -> Option<PointerValue<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(*val);
            }
        }
        None
    }
}
