use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::interpreter::RuntimeValue;
use crate::token::Token;

pub type EnvRef = Rc<RefCell<Environment>>;

pub struct Environment {
    values: HashMap<String, RuntimeValue>,
    enclosing: Option<EnvRef>,
}

impl Environment {
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: None,
        }))
    }

    pub fn with_enclosing(enclosing: &EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: Some(Rc::clone(enclosing)),
        }))
    }

    pub fn define(&mut self, name: &str, value: RuntimeValue) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &Token) -> Result<RuntimeValue, String> {
        if let Some(val) = self.values.get(&name.lexeme) {
            return Ok(val.clone());
        }

        if let Some(ref enc) = self.enclosing {
            return enc.borrow().get(name);
        }

        Err(format!("Undefined variable '{}'.", name.lexeme))
    }

    pub fn assign(&mut self, name: &Token, value: RuntimeValue) -> Result<(), String> {
        if self.values.contains_key(&name.lexeme) {
            self.values.insert(name.lexeme.clone(), value);
            return Ok(());
        }

        if let Some(ref enc) = self.enclosing {
            return enc.borrow_mut().assign(name, value);
        }

        Err(format!("Undefined variable '{}'.", name.lexeme))
    }
}
