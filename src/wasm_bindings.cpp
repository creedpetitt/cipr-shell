#include <emscripten/bind.h>
#include <iostream>
#include <sstream>
#include "Core/Core.h"
#include "Scanner/Scanner.h"
#include "Parser/Parser.h"
#include "AST/AstPrinter.h"

std::string run_cipr_code(const std::string& code) {
    std::ostringstream oss;
    std::streambuf* old_cout = std::cout.rdbuf(oss.rdbuf());
    std::streambuf* old_cerr = std::cerr.rdbuf(oss.rdbuf());

    Core core;
    Core::hadError = false; 
    core.run(code);

    std::cout.rdbuf(old_cout);
    std::cerr.rdbuf(old_cerr);

    return oss.str();
}

std::string get_cipr_ast(const std::string& code) {
    Arena arena;
    Core::hadError = false;
    
    Scanner scanner(code);
    const std::vector<Token> tokens = scanner.scanTokens();

    Parser parser(tokens, arena);
    const int rootIndex = parser.parse();

    if (Core::hadError) {
        return "Error parsing AST.";
    }

    AstPrinter printer(arena);
    return printer.print(rootIndex);
}

EMSCRIPTEN_BINDINGS(cipr_module) {
    emscripten::function("runCode", &run_cipr_code);
    emscripten::function("getAst", &get_cipr_ast);
}
