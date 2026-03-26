use std::fmt;
use std::rc::Rc;

use crate::ast::{NodeArena, NodeId, NodeType};
use crate::environment::{EnvRef, Environment};
use crate::token::{Token, TokenType, Value};

// ── Runtime Value ──
// Separate from token::Value. This is the *runtime* representation that
// includes callables and arrays. Plain data at scan/parse time, rich objects
// at runtime.

#[derive(Clone)]
pub enum RuntimeValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Callable(Rc<dyn Callable>),
    Array(Vec<RuntimeValue>),
}

impl RuntimeValue {
    pub fn from_parse_value(v: &Value) -> Self {
        match v {
            Value::Null => RuntimeValue::Null,
            Value::Bool(b) => RuntimeValue::Bool(*b),
            Value::Int(n) => RuntimeValue::Int(*n),
            Value::Float(n) => RuntimeValue::Float(*n),
            Value::Str(s) => RuntimeValue::Str(s.clone()),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            RuntimeValue::Null => false,
            RuntimeValue::Bool(b) => *b,
            RuntimeValue::Int(n) => *n != 0,
            RuntimeValue::Float(n) => *n != 0.0,
            _ => true,
        }
    }
}

impl PartialEq for RuntimeValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RuntimeValue::Null, RuntimeValue::Null) => true,
            (RuntimeValue::Bool(a), RuntimeValue::Bool(b)) => a == b,
            (RuntimeValue::Int(a), RuntimeValue::Int(b)) => a == b,
            (RuntimeValue::Float(a), RuntimeValue::Float(b)) => a == b,
            (RuntimeValue::Str(a), RuntimeValue::Str(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for RuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", stringify_value(self))
    }
}

pub fn stringify_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Null => "null".to_string(),
        RuntimeValue::Bool(b) => format!("{b}"),
        RuntimeValue::Int(n) => format!("{n}"),
        RuntimeValue::Float(n) => {
            if *n == n.trunc() && n.is_finite() {
                format!("{n:.0}")
            } else {
                format!("{n}")
            }
        }
        RuntimeValue::Str(s) => s.clone(),
        RuntimeValue::Callable(c) => c.to_string(),
        RuntimeValue::Array(elems) => {
            let parts: Vec<String> = elems.iter().map(stringify_value).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

// ── Callable Trait ──

pub trait Callable {
    fn arity(&self) -> usize;
    fn call(
        &self,
        interp: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError>;
    fn to_string(&self) -> String;
}

// ── Error Types ──

pub enum CiprError {
    RuntimeError { token: Token, message: String },
    Return(RuntimeValue),
}

impl fmt::Display for CiprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CiprError::RuntimeError { message, .. } => write!(f, "{message}"),
            CiprError::Return(_) => write!(f, "return"),
        }
    }
}

// ── Named Function (stores name + arity at construction) ──

pub struct NamedFunction {
    pub name: String,
    pub param_count: usize,
    declaration_id: NodeId,
    closure: EnvRef,
}

impl NamedFunction {
    pub fn new(name: String, param_count: usize, declaration_id: NodeId, closure: EnvRef) -> Self {
        Self {
            name,
            param_count,
            declaration_id,
            closure,
        }
    }
}

impl Callable for NamedFunction {
    fn arity(&self) -> usize {
        self.param_count
    }

    fn call(
        &self,
        interp: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let env = Environment::with_enclosing(&self.closure);

        let children: Vec<Option<NodeId>> = interp.arena[self.declaration_id].children.clone();

        for i in 0..self.param_count {
            if let Some(param_id) = children[i] {
                let param_name = interp.arena[param_id].token.lexeme.clone();
                env.borrow_mut().define(&param_name, args[i].clone());
            }
        }

        let body_id = children[children.len() - 1];
        if let Some(body) = body_id {
            let body_children: Vec<Option<NodeId>> = interp.arena[body].children.clone();
            match interp.execute_block(&body_children, &env) {
                Ok(()) => Ok(RuntimeValue::Null),
                Err(CiprError::Return(val)) => Ok(val),
                Err(e) => Err(e),
            }
        } else {
            Ok(RuntimeValue::Null)
        }
    }

    fn to_string(&self) -> String {
        format!("<fn {}>", self.name)
    }
}

// ── Interpreter ──

pub struct Interpreter {
    pub arena: NodeArena,
    environment: EnvRef,
}

impl Interpreter {
    pub fn new(arena: NodeArena) -> Self {
        let globals = Environment::new();
        let environment = Rc::clone(&globals);

        let interp = Self {
            arena,
            environment,
        };
        interp
    }

    pub fn interpret(&mut self, root: NodeId) {
        if let Err(e) = self.execute(root) {
            match e {
                CiprError::RuntimeError {
                    ref token,
                    ref message,
                } => {
                    eprintln!("Runtime Error: {message}\n[line {}]", token.line);
                }
                CiprError::Return(_) => {}
            }
        }
    }

    fn execute(&mut self, id: NodeId) -> Result<(), CiprError> {
        let node_type = self.arena[id].node_type;

        match node_type {
            NodeType::StmtList => self.visit_stmt_list(id),
            NodeType::StmtVarDecl => self.visit_var_declaration(id),
            NodeType::StmtExpr => self.visit_expression_stmt(id),
            NodeType::StmtBlock => self.visit_block_stmt(id),
            NodeType::StmtIf => self.visit_if_stmt(id),
            NodeType::StmtWhile => self.visit_while_stmt(id),
            NodeType::StmtFunction => self.visit_function_stmt(id),
            NodeType::StmtReturn => self.visit_return_stmt(id),
            _ => {
                self.evaluate(id)?;
                Ok(())
            }
        }
    }

    pub fn execute_block(
        &mut self,
        statements: &[Option<NodeId>],
        env: &EnvRef,
    ) -> Result<(), CiprError> {
        let previous = Rc::clone(&self.environment);
        self.environment = Rc::clone(env);

        let result = (|| {
            for id in statements.iter().flatten() {
                self.execute(*id)?;
            }
            Ok(())
        })();

        self.environment = previous;
        result
    }

    // ── Statement visitors ──

    fn visit_stmt_list(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        for child_id in children.iter().flatten() {
            self.execute(*child_id)?;
        }
        Ok(())
    }

    fn visit_var_declaration(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let name = self.arena[id].token.lexeme.clone();

        let value = if !children.is_empty() {
            if let Some(init_id) = children[0] {
                self.evaluate(init_id)?
            } else {
                RuntimeValue::Null
            }
        } else {
            RuntimeValue::Null
        };

        self.environment.borrow_mut().define(&name, value);
        Ok(())
    }

    fn visit_expression_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let child = self.arena[id].children[0];
        if let Some(expr_id) = child {
            self.evaluate(expr_id)?;
        }
        Ok(())
    }

    fn visit_block_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let block_env = Environment::with_enclosing(&self.environment);
        self.execute_block(&children, &block_env)
    }

    fn visit_if_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let condition = children[0];
        let then_branch = children[1];
        let else_branch = children.get(2).copied().flatten();

        if let Some(cond_id) = condition {
            let val = self.evaluate(cond_id)?;
            if val.is_truthy() {
                if let Some(then_id) = then_branch {
                    self.execute(then_id)?;
                }
            } else if let Some(else_id) = else_branch {
                self.execute(else_id)?;
            }
        }

        Ok(())
    }

    fn visit_while_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let condition = children[0];
        let body = children[1];

        if let (Some(cond_id), Some(body_id)) = (condition, body) {
            loop {
                let val = self.evaluate(cond_id)?;
                if !val.is_truthy() {
                    break;
                }
                self.execute(body_id)?;
            }
        }

        Ok(())
    }

    fn visit_function_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let name = self.arena[id].token.lexeme.clone();
        let param_count = self.arena[id].children.len() - 1;
        let func = NamedFunction::new(name.clone(), param_count, id, Rc::clone(&self.environment));
        self.environment
            .borrow_mut()
            .define(&name, RuntimeValue::Callable(Rc::new(func)));
        Ok(())
    }

    fn visit_return_stmt(&mut self, id: NodeId) -> Result<(), CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let value = if !children.is_empty() {
            if let Some(val_id) = children[0] {
                self.evaluate(val_id)?
            } else {
                RuntimeValue::Null
            }
        } else {
            RuntimeValue::Null
        };

        Err(CiprError::Return(value))
    }

    // ── Expression visitors ──

    fn evaluate(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let node_type = self.arena[id].node_type;

        match node_type {
            NodeType::Literal => self.visit_literal(id),
            NodeType::Grouping => self.visit_grouping(id),
            NodeType::Unary => self.visit_unary(id),
            NodeType::Binary => self.visit_binary(id),
            NodeType::VarExpr => self.visit_var_expr(id),
            NodeType::Assign => self.visit_assignment_expr(id),
            NodeType::Logical => self.visit_logical_expr(id),
            NodeType::Call => self.visit_call_expr(id),
            NodeType::Array => self.visit_array_expr(id),
            NodeType::IndexGet => self.visit_index_get(id),
            _ => Ok(RuntimeValue::Null),
        }
    }

    fn visit_literal(&self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        Ok(RuntimeValue::from_parse_value(&self.arena[id].value))
    }

    fn visit_grouping(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let child = self.arena[id].children[0];
        match child {
            Some(c) => self.evaluate(c),
            None => Ok(RuntimeValue::Null),
        }
    }

    fn visit_unary(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let child = self.arena[id].children[0];
        let op_type = self.arena[id].token.token_type;

        let right = match child {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        match op_type {
            TokenType::Minus => match right {
                RuntimeValue::Int(n) => Ok(RuntimeValue::Int(-n)),
                RuntimeValue::Float(n) => Ok(RuntimeValue::Float(-n)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Bang => Ok(RuntimeValue::Bool(!right.is_truthy())),
            _ => Ok(RuntimeValue::Null),
        }
    }

    fn visit_binary(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let op_type = self.arena[id].token.token_type;
        let op_token = self.arena[id].token.clone();

        let left = match children[0] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };
        let right = match children[1] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        match op_type {
            TokenType::Minus => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Int(l - r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Float(l - r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Slash => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => {
                    if *r == 0 {
                        return Err(CiprError::RuntimeError {
                            token: op_token,
                            message: "Division by zero.".to_string(),
                        });
                    }
                    Ok(RuntimeValue::Int(l / r))
                }
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => {
                    if *r == 0.0 {
                        return Err(CiprError::RuntimeError {
                            token: op_token,
                            message: "Division by zero.".to_string(),
                        });
                    }
                    Ok(RuntimeValue::Float(l / r))
                }
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Star => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Int(l * r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Float(l * r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Plus => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Int(l + r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Float(l + r)),
                (RuntimeValue::Str(_), _) | (_, RuntimeValue::Str(_)) => Ok(RuntimeValue::Str(
                    format!("{}{}", stringify_value(&left), stringify_value(&right)),
                )),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Greater => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Bool(l > r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Bool(l > r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::GreaterEqual => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Bool(l >= r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Bool(l >= r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::Less => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Bool(l < r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Bool(l < r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::LessEqual => match (&left, &right) {
                (RuntimeValue::Int(l), RuntimeValue::Int(r)) => Ok(RuntimeValue::Bool(l <= r)),
                (RuntimeValue::Float(l), RuntimeValue::Float(r)) => Ok(RuntimeValue::Bool(l <= r)),
                _ => unreachable!(
                    "Type checker failed: Unhandled cross-type arithmetic reached the interpreter!"
                ),
            },
            TokenType::BangEqual => Ok(RuntimeValue::Bool(left != right)),
            TokenType::EqualEqual => Ok(RuntimeValue::Bool(left == right)),
            _ => Ok(RuntimeValue::Null),
        }
    }

    fn visit_var_expr(&self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let token = self.arena[id].token.clone();
        self.environment
            .borrow()
            .get(&token)
            .map_err(|msg| CiprError::RuntimeError {
                token,
                message: msg,
            })
    }

    fn visit_assignment_expr(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let token = self.arena[id].token.clone();

        let value = match children[0] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        self.environment
            .borrow_mut()
            .assign(&token, value.clone())
            .map_err(|msg| CiprError::RuntimeError {
                token,
                message: msg,
            })?;

        Ok(value)
    }

    fn visit_logical_expr(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let op_type = self.arena[id].token.token_type;

        let left = match children[0] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        if op_type == TokenType::Or && left.is_truthy() {
            return Ok(left);
        }
        if op_type == TokenType::And && !left.is_truthy() {
            return Ok(left);
        }

        match children[1] {
            Some(c) => self.evaluate(c),
            None => Ok(RuntimeValue::Null),
        }
    }

    fn visit_call_expr(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let op_token = self.arena[id].token.clone();

        let callee = match children[0] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        let mut arguments = Vec::new();
        for c in children[1..].iter().flatten() {
            arguments.push(self.evaluate(*c)?);
        }

        let function = match callee {
            RuntimeValue::Callable(ref f) => Rc::clone(f),
            _ => {
                return Err(CiprError::RuntimeError {
                    token: op_token,
                    message: "Can only call functions and classes.".to_string(),
                });
            }
        };

        let arity = function.arity();
        if arity != usize::MAX && arguments.len() != arity {
            return Err(CiprError::RuntimeError {
                token: op_token,
                message: format!("Expected {} arguments but got {}.", arity, arguments.len()),
            });
        }

        function.call(self, arguments)
    }

    fn visit_array_expr(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let mut elements = Vec::new();
        for c in children.iter().flatten() {
            elements.push(self.evaluate(*c)?);
        }
        Ok(RuntimeValue::Array(elements))
    }

    fn visit_index_get(&mut self, id: NodeId) -> Result<RuntimeValue, CiprError> {
        let children: Vec<Option<NodeId>> = self.arena[id].children.clone();
        let op_token = self.arena[id].token.clone();

        let target = match children[0] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };
        let index = match children[1] {
            Some(c) => self.evaluate(c)?,
            None => RuntimeValue::Null,
        };

        let arr = match target {
            RuntimeValue::Array(ref elems) => elems,
            _ => {
                return Err(CiprError::RuntimeError {
                    token: op_token,
                    message: "Only arrays can be indexed.".to_string(),
                });
            }
        };

        let i = match index {
            RuntimeValue::Int(n) => n as usize,
            _ => {
                return Err(CiprError::RuntimeError {
                    token: op_token,
                    message: "Index must be an int.".to_string(),
                });
            }
        };

        if i >= arr.len() {
            return Err(CiprError::RuntimeError {
                token: op_token,
                message: "Array index out of bounds.".to_string(),
            });
        }

        Ok(arr[i].clone())
    }
}
