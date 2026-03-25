use crate::ast::{NodeArena, NodeId, NodeType};
use crate::token::Value;

pub struct AstPrinter<'a> {
    arena: &'a NodeArena,
}

impl<'a> AstPrinter<'a> {
    pub fn new(arena: &'a NodeArena) -> Self {
        Self { arena }
    }

    pub fn print(&self, node_id: Option<NodeId>) -> String {
        let node_id = match node_id {
            Some(id) => id,
            None => return String::new(),
        };

        let node = &self.arena[node_id];
        match node.node_type {
            NodeType::Literal => match &node.value {
                Value::Null => "null".to_string(),
                Value::Int(n) => format!("{n}"),
                Value::Float(n) => format!("{n}"),
                Value::Str(s) => s.clone(),
                Value::Bool(b) => format!("{b}"),
            },
            NodeType::Unary => {
                let lexeme = node.token.lexeme.clone();
                self.parenthesize(&lexeme, &[node.children[0]])
            }
            NodeType::Binary | NodeType::Logical => {
                let lexeme = node.token.lexeme.clone();
                let children = [node.children[0], node.children[1]];
                self.parenthesize(&lexeme, &children)
            }
            NodeType::Grouping => self.parenthesize("group", &[node.children[0]]),
            NodeType::StmtList => {
                let children: Vec<_> = node.children.clone();
                self.parenthesize("list", &children)
            }
            NodeType::StmtExpr => self.parenthesize("expr", &[node.children[0]]),
            NodeType::StmtBlock => {
                let children: Vec<_> = node.children.clone();
                self.parenthesize("block", &children)
            }
            NodeType::StmtVarDecl => {
                let name = node.token.lexeme.clone();
                let children: Vec<_> = node.children.clone();
                self.parenthesize(&format!("var {name}"), &children)
            }
            NodeType::VarExpr => node.token.lexeme.clone(),
            NodeType::Assign => {
                let name = node.token.lexeme.clone();
                let children: Vec<_> = node.children.clone();
                self.parenthesize(&format!("assign {name}"), &children)
            }
            NodeType::StmtIf
            | NodeType::StmtWhile
            | NodeType::Call
            | NodeType::Array
            | NodeType::IndexGet
            | NodeType::StmtReturn => {
                let tag = match node.node_type {
                    NodeType::StmtIf => "if",
                    NodeType::StmtWhile => "while",
                    NodeType::Call => "call",
                    NodeType::Array => "array",
                    NodeType::IndexGet => "index",
                    NodeType::StmtReturn => "return",
                    _ => unreachable!(),
                };
                let children: Vec<_> = node.children.clone();
                self.parenthesize(tag, &children)
            }
            NodeType::StmtFunction => {
                let name = node.token.lexeme.clone();
                let children: Vec<_> = node.children.clone();
                self.parenthesize(&format!("fn {name}"), &children)
            }
            NodeType::AddressOf => {
                self.parenthesize("@", &[node.children[0]])
            }
            NodeType::Dereference => {
                self.parenthesize("deref", &[node.children[0]])
            }
            NodeType::AssignDeref => {
                let children = [node.children[0], node.children[1]];
                self.parenthesize("assign_deref", &children)
            }
            NodeType::StmtStructDecl => {
                let name = node.token.lexeme.clone();
                self.parenthesize(&format!("struct {name}"), &node.children)
            }
            NodeType::StructInit => {
                let prev = node.token.lexeme.clone();
                self.parenthesize(&format!("init {prev}"), &node.children)
            }
            NodeType::GetField => {
                let name = node.token.lexeme.clone();
                self.parenthesize(&format!("get {name}"), &[node.children[0]])
            }
            NodeType::AssignField => {
                let name = node.token.lexeme.clone();
                let children = [node.children[0], node.children[1]];
                self.parenthesize(&format!("set {name}"), &children)
            }
        }
    }

    fn parenthesize(&self, name: &str, children: &[Option<NodeId>]) -> String {
        let mut result = format!("({name}");
        for child in children {
            if child.is_some() {
                result.push(' ');
                result.push_str(&self.print(*child));
            }
        }
        result.push(')');
        result
    }
}
