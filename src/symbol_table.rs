use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use inkwell::values::PointerValue;

pub type SymbolTableRef<'ctx> = Rc<RefCell<SymbolTable<'ctx>>>;

pub struct SymbolTable<'ctx> {
    values: HashMap<String, PointerValue<'ctx>>,
    enclosing: Option<SymbolTableRef<'ctx>>,
}

impl<'ctx> SymbolTable<'ctx> {
    pub fn new() -> SymbolTableRef<'ctx> {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: None,
        }))
    }

    pub fn with_enclosing(enclosing: &SymbolTableRef<'ctx>) -> SymbolTableRef<'ctx> {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: Some(Rc::clone(enclosing)),
        }))
    }

    pub fn define(&mut self, name: &str, pointer: PointerValue<'ctx>) {
        self.values.insert(name.to_string(), pointer);
    }

    pub fn get(&self, name: &str) -> Option<PointerValue<'ctx>> {
        if let Some(val) = self.values.get(name) {
            return Some(*val);
        }

        if let Some(ref enc) = self.enclosing {
            return enc.borrow().get(name);
        }

        None
    }
}
