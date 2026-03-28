use crate::ast::CiprType;

pub fn parse_annotation(annotation: &Option<CiprType>) -> CiprType {
    annotation.clone().unwrap_or(CiprType::Unknown)
}

pub fn parse_named_type(name: &str) -> CiprType {
    match name {
        "int" => CiprType::Int,
        "float" => CiprType::Float,
        "str" => CiprType::Str,
        "bool" => CiprType::Bool,
        "void" => CiprType::Void,
        _ => CiprType::Struct(name.to_string()),
    }
}
