//
// Created by creed on 1/5/26.
//

#include "AstPrinter.h"
#include <sstream>

std::string AstPrinter::print(const int nodeIdx) {
    if (nodeIdx == -1) return "";

    switch (Node& node = arena.get(nodeIdx); node.type) {
        case NodeType::LITERAL: {
            if (std::holds_alternative<std::monostate>(node.value))
                return "null";

            if (std::holds_alternative<double>(node.value)) {
                std::ostringstream oss;
                oss << std::get<double>(node.value);
                return oss.str();
            }

            if (std::holds_alternative<std::string>(node.value))
                return std::get<std::string>(node.value);

            return "";
        }
        case NodeType::UNARY:
            return parenthesize(node.op.lexeme, {node.children[0]});
        case NodeType::BINARY:
            return parenthesize(node.op.lexeme, {node.children[0], node.children[1]});
        case NodeType::GROUPING:
            return parenthesize("group", {node.children[0]});
        case NodeType::STMT_LIST:
            return parenthesize("list", node.children);
        case NodeType::STMT_ECHO:
            return parenthesize("echo", {node.children[0]});
        case NodeType::STMT_EXPR:
            return parenthesize("expr", {node.children[0]});
        case NodeType::STMT_BLOCK:
            return parenthesize("block", node.children);
        case NodeType::STMT_VAR_DECL:
            return parenthesize("var " + node.op.lexeme, node.children);
        case NodeType::VAR_EXPR:
            return node.op.lexeme;
        case NodeType::ASSIGN:
            return parenthesize("assign " + node.op.lexeme, node.children);
        case NodeType::LOGICAL:
            return parenthesize(node.op.lexeme, {node.children[0], node.children[1]});
        case NodeType::STMT_IF:
            return parenthesize("if", node.children);
        case NodeType::STMT_WHILE:
            return parenthesize("while", node.children);
        case NodeType::CALL:
            return parenthesize("call", node.children);
        case NodeType::ARRAY:
            return parenthesize("array", node.children);
        case NodeType::INDEX_GET:
            return parenthesize("index", node.children);
        case NodeType::STMT_FUNCTION:
            return parenthesize("fn " + node.op.lexeme, node.children);
        case NodeType::STMT_RETURN:
            return parenthesize("return", node.children);
    }
    return "";
}

std::string AstPrinter::parenthesize(const std::string& name, const std::vector<int>& indices) {
    std::string result = "(" + name;
    for (int index : indices) {
        if (index != -1) result += " " + print(index);
    }
    result += ")";
    return result;
}