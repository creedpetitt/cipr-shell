//
// Created by creed on 1/5/26.
//

#include "Interpreter.h"

#include "Function.h"
#include "Common/RuntimeError.h"
#include "Common/Return.h"
#include "Native/NativeRegistry.h"

#include <sstream>

Interpreter::Interpreter(Arena& arena) : arena(arena) {
    globals = std::make_shared<Environment>();
    environment = globals;
    NativeRegistry::registerAll(globals);
}

void Interpreter::interpret(const int rootIndex) {
    try {
        execute(rootIndex);
    } catch (const RuntimeError& error) {
        std::cerr << "Runtime Error: " << error.what() << "\n[line " << error.token.line << "]" << std::endl;
    }
}

void Interpreter::execute(const int index) {
    switch (const Node& node = arena.get(index); node.type) {
        case NodeType::STMT_LIST:
            visitStmtList(node);
            break;
        case NodeType::STMT_VAR_DECL:
            visitVarDeclaration(node);
            break;
        case NodeType::STMT_ECHO:
            visitEchoStmt(node);
            break;
        case NodeType::STMT_EXPR:
            visitExpressionStmt(node);
            break;
        case NodeType::STMT_BLOCK:
            visitBlockStmt(node);
            break;
        case NodeType::STMT_IF:
            visitIfStmt(node);
            break;
        case NodeType::STMT_WHILE:
            visitWhileStmt(node);
            break;
        case NodeType::STMT_FUNCTION:
            visitFunctionStmt(node, index);
            break;
        case NodeType::STMT_RETURN:
            visitReturnStmt(node);
            break;
        default:
            evaluate(index);
            break;
    }
}

void Interpreter::executeBlock(const std::vector<int>& statements,
    const std::shared_ptr<Environment> &env) {

    const std::shared_ptr<Environment> previous = this->environment;

    try {
        this->environment = env;
           for (const int index : statements) {
               execute(index);
             }
    } catch (...) {
        this->environment = previous;
        throw;
    }

    this->environment = previous;
}

void Interpreter::visitBlockStmt(const Node& node) {
    const auto blockEnv = std::make_shared<Environment>(environment);

    executeBlock(node.children, blockEnv);
}

void Interpreter::visitWhileStmt(const Node& node) {
    while (isTruthy(evaluate(node.children[0]))) {
        execute(node.children[1]);
    }
}

void Interpreter::visitIfStmt(const Node& node) {
    if (isTruthy(evaluate(node.children[0]))) {
        execute(node.children[1]);
    } else if (node.children[2] != -1) {
        execute(node.children[2]);
    }
}

void Interpreter::visitFunctionStmt(const Node& node, int index) {
    auto function = std::make_shared<Function>(index, arena, environment);
    environment->define(node.op.lexeme, function);
}

void Interpreter::visitReturnStmt(const Node& node) {
    Literal value = std::monostate{};
    if (!node.children.empty() && node.children[0] != -1) {
        value = evaluate(node.children[0]);
    }
    throw Return(value);
}

void Interpreter::visitStmtList(const Node& node) {
    for (const int childIndex : node.children) {
        execute(childIndex);
    }
}

void Interpreter::visitEchoStmt(const Node& node) {
    const Literal value = evaluate(node.children[0]);
    std::cout << stringify(value) << std::endl;
}

void Interpreter::visitExpressionStmt(const Node& node) {
    evaluate(node.children[0]);
}

void Interpreter::visitVarDeclaration(const Node &node) {
    Literal value = std::monostate{};

    if (!node.children.empty() && node.children[0] != -1) {
        value = evaluate(node.children[0]);
    }
    environment->define(node.op.lexeme, value);
}


Literal Interpreter::evaluate(const int index) {
    if (index == -1)
        return std::monostate{};

    switch (const Node& node = arena.get(index); node.type) {
        case NodeType::VAR_EXPR:
            return visitVarExpr(node);
        case NodeType::ASSIGN:
           return visitAssignmentExpr(node);
        case NodeType::LOGICAL:
            return visitLogicalExpr(node);
        case NodeType::CALL:
            return visitCallExpr(node);
        case NodeType::ARRAY:
            return visitArrayExpr(node);
        case NodeType::INDEX_GET:
            return visitIndexGet(node);
        case NodeType::LITERAL:
            return visitLiteral(node);
        case NodeType::GROUPING:
            return visitGrouping(node);
        case NodeType::UNARY:
            return visitUnary(node);
        case NodeType::BINARY:
            return visitBinary(node);
        default:
            return std::monostate{};
    }
}

Literal Interpreter::visitLogicalExpr(const Node &node) {
    Literal left = evaluate(node.children[0]);

    if (node.op.type == OR) {
        if (isTruthy(left))
            return left;
    }

    if (node.op.type == AND) {
        if (!isTruthy(left))
            return left;
    }

    return evaluate(node.children[1]);
}

Literal Interpreter::visitCallExpr(const Node& node) {
    const Literal callee = evaluate(node.children[0]);

    std::vector<Literal> arguments;
    for (size_t i = 1; i < node.children.size(); i++) {
        arguments.push_back(evaluate(node.children[i]));
    }

    if (!std::holds_alternative<std::shared_ptr<Callable>>(callee)) {
        throw RuntimeError(node.op, "Can only call functions and classes.");
    }

    const auto function = std::get<std::shared_ptr<Callable>>(callee);

    if (arguments.size() != static_cast<size_t>(function->arity())) {
        throw RuntimeError(node.op, "Expected " +
            std::to_string(function->arity()) + " arguments but got " +
            std::to_string(arguments.size()) + ".");
    }

    return function->call(*this, arguments);
}

Literal Interpreter::visitArrayExpr(const Node& node) {
    auto list = std::make_shared<LiteralVector>();
    for (const int childIdx : node.children) {
        list->elements.push_back(evaluate(childIdx));
    }
    return list;
}

Literal Interpreter::visitIndexGet(const Node& node) {
    const Literal target = evaluate(node.children[0]);
    const Literal index = evaluate(node.children[1]);

    if (!std::holds_alternative<std::shared_ptr<LiteralVector>>(target)) {
        throw RuntimeError(node.op, "Only arrays can be indexed.");
    }

    if (!std::holds_alternative<double>(index)) {
        throw RuntimeError(node.op, "Index must be a number.");
    }

    const auto list = std::get<std::shared_ptr<LiteralVector>>(target);
    const int i = static_cast<int>(std::get<double>(index));

    if (i < 0 || i >= list->elements.size()) {
        throw RuntimeError(node.op, "Array index out of bounds.");
    }

    return list->elements[i];
}

Literal Interpreter::visitVarExpr(const Node &node) const {
    return environment->get(node.op);
}

Literal Interpreter::visitAssignmentExpr(const Node &node) {
    Literal value = evaluate(node.children[0]);
    environment->assign(node.op, value);
    return value;
}

Literal Interpreter::visitLiteral(const Node& node) {
    return node.value;
}

Literal Interpreter::visitGrouping(const Node& node) {
    return evaluate(node.children[0]);
}

Literal Interpreter::visitUnary(const Node& node) {
    const Literal right = evaluate(node.children[0]);

    switch (node.op.type) {
        case MINUS:
            checkNumberOperand(node.op, right);
            return -std::get<double>(right);

        case BANG:
            return !isTruthy(right);

        default:
            return std::monostate{};
    }
}

Literal Interpreter::visitBinary(const Node& node) {
    const Literal left = evaluate(node.children[0]);
    const Literal right = evaluate(node.children[1]);

    switch (node.op.type) {
        case MINUS:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) - std::get<double>(right);
        case SLASH:
            checkNumberOperands(node.op, left, right);
            if (std::get<double>(right) == 0.0) {
                throw RuntimeError(node.op, "Division by zero.");
            }
            return std::get<double>(left) / std::get<double>(right);
        case STAR:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) * std::get<double>(right);

        case PLUS:
            if (std::holds_alternative<double>(left) && std::holds_alternative<double>(right)) {
                return std::get<double>(left) + std::get<double>(right);
            }
            if (std::holds_alternative<std::string>(left) || std::holds_alternative<std::string>(right)) {
                return stringify(left) + stringify(right);
            }

            throw RuntimeError(node.op, "Operands must be two numbers or two strings.");

        case GREATER:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) > std::get<double>(right);
        case GREATER_EQUAL:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) >= std::get<double>(right);
        case LESS:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) < std::get<double>(right);
        case LESS_EQUAL:
            checkNumberOperands(node.op, left, right);
            return std::get<double>(left) <= std::get<double>(right);

        case BANG_EQUAL: return !isEqual(left, right);
        case EQUAL_EQUAL: return isEqual(left, right);

        default: return std::monostate{};
    }
}

bool Interpreter::isTruthy(const Literal& value) {
    if (std::holds_alternative<std::monostate>(value))
        return false;

    if (std::holds_alternative<bool>(value))
        return std::get<bool>(value);

    if (std::holds_alternative<double>(value)) {
        return std::get<double>(value) != 0.0;
    }

    return true;
}

void Interpreter::checkNumberOperand(const Token& op, const Literal& operand) {
    if (std::holds_alternative<double>(operand))
        return;
    throw RuntimeError(op, "Operand must be a number.");
}

void Interpreter::checkNumberOperands(const Token& op, const Literal& left, const Literal& right) {
    if (std::holds_alternative<double>(left) && std::holds_alternative<double>(right))
        return;
    throw RuntimeError(op, "Operands must be numbers.");
}

bool Interpreter::isEqual(const Literal& a, const Literal& b) {
    if (a.index() != b.index())
        return false;
    return a == b;
}

std::string Interpreter::stringify(const Literal& value) {
    if (std::holds_alternative<std::monostate>(value)) return "null";
    if (std::holds_alternative<bool>(value)) return std::get<bool>(value) ? "true" : "false";

    if (std::holds_alternative<double>(value)) {
        std::ostringstream oss;
        oss << std::get<double>(value);
        return oss.str();
    }

    if (std::holds_alternative<std::string>(value)) return std::get<std::string>(value);

    if (std::holds_alternative<std::shared_ptr<Callable>>(value)) {
        return std::get<std::shared_ptr<Callable>>(value)->toString();
    }

    if (std::holds_alternative<std::shared_ptr<LiteralVector>>(value)) {
        std::string result = "[";
        const auto list = std::get<std::shared_ptr<LiteralVector>>(value);
        for (size_t i = 0; i < list->elements.size(); ++i) {
            result += stringify(list->elements[i]);
            if (i < list->elements.size() - 1) result += ", ";
        }
        result += "]";
        return result;
    }

    return "unknown";
}