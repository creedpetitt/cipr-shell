use id_arena::{Arena, Id};

use crate::token::{Token, Value};

pub type NodeId = Id<Node>;
pub type NodeArena = Arena<Node>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Binary,
    Grouping,
    Literal,
    Unary,
    Logical,
    Assign,
    VarExpr,
    Call,
    StmtList,
    StmtExpr,
    StmtVarDecl,
    StmtBlock,
    StmtIf,
    StmtWhile,
    StmtFunction,
    StmtReturn,
    Array,
    IndexGet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiprType {
    Int,
    Float,
    Str,
    Bool,
    Void,
    Array(Box<CiprType>),
    Callable(Vec<CiprType>, Box<CiprType>),
    Unknown,
}

pub struct Node {
    pub node_type: NodeType,
    pub token: Token,
    pub value: Value,
    pub children: Vec<Option<NodeId>>,
    pub type_annotation: Option<String>,
    pub resolved_type: CiprType,
}

impl Node {
    pub fn new(
        node_type: NodeType,
        token: Token,
        value: Value,
        children: Vec<Option<NodeId>>,
        type_annotation: Option<String>,
    ) -> Self {
        Self {
            node_type,
            token,
            value,
            children,
            type_annotation,
            resolved_type: CiprType::Unknown,
        }
    }
}

pub fn alloc_node(
    arena: &mut NodeArena,
    node_type: NodeType,
    token: Token,
    value: Value,
    children: Vec<Option<NodeId>>,
) -> NodeId {
    arena.alloc(Node::new(node_type, token, value, children, None))
}

pub fn alloc_node_typed(
    arena: &mut NodeArena,
    node_type: NodeType,
    token: Token,
    value: Value,
    children: Vec<Option<NodeId>>,
    type_annotation: Option<String>,
) -> NodeId {
    arena.alloc(Node::new(
        node_type,
        token,
        value,
        children,
        type_annotation,
    ))
}
