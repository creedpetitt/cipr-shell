#ifndef CIPR_NATIVE_CORE_H
#define CIPR_NATIVE_CORE_H

#include "Interpreter/Interpreter.h"
#include "Interpreter/Callable.h"
#include "Scanner/Scanner.h"
#include "Parser/Parser.h"
#include <ctime>
#include <cstdio>
#include <memory>
#include <array>
#include <string>
#include <unistd.h>
#include <climits>
#include <thread>
#include <chrono>
#include <fstream>
#include <sstream>
#include <filesystem>
#include <random>

struct NativeTime final : Callable {
    int arity() override {
        return 0;
    }

    Literal call(Interpreter&, std::vector<Literal>) override {
        const auto now = std::chrono::system_clock::now();
        const auto duration = now.time_since_epoch();
        return std::chrono::duration<double>(duration).count();
    }

    std::string toString() override {
        return "<native fn time>";
    }
};

#ifndef __EMSCRIPTEN__
struct NativeRun final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        if (!std::holds_alternative<std::string>(args[0]))
            return std::monostate{};
        const auto cmd = std::get<std::string>(args[0]);
        std::array<char, 128> buf{};
        std::string res;
        const std::unique_ptr<FILE, decltype(&pclose)> pipe(popen(cmd.c_str(), "r"), pclose);
        if (!pipe)
            return std::string("Error: Pipe failed");

        while (fgets(buf.data(), buf.size(), pipe.get()) != nullptr)
            res += buf.data();
        return res;
    }

    std::string toString() override {
        return "<native fn run>";
    }
};
#endif

struct NativeEnv final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        if (!std::holds_alternative<std::string>(args[0]))
            return std::monostate{};

        const char* val = std::getenv(std::get<std::string>(args[0]).c_str());
        if (val)
            return std::string(val);
        return std::monostate{};
    }

    std::string toString() override {
        return "<native fn env>";
    }
};

struct NativeCwd final : Callable {
    int arity() override {
        return 0;
    }

    Literal call(Interpreter&, std::vector<Literal>) override {
        char cwd[PATH_MAX];
        if (getcwd(cwd, sizeof(cwd)) != nullptr) {
            return std::string(cwd);
        }
        return std::monostate{};
    }

    std::string toString() override {
        return "<native fn cwd>";
    }
};

struct NativeCd final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        if (!std::holds_alternative<std::string>(args[0]))
            return false;
        const std::string path = std::get<std::string>(args[0]);
        return chdir(path.c_str()) == 0;
    }

    std::string toString() override {
        return "<native fn cd>";
    }
};

struct NativeInclude final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter& interpreter, std::vector<Literal> args) override {
        if (!std::holds_alternative<std::string>(args[0]))
            return false;
        auto filename = std::get<std::string>(args[0]);
        
        // 1. Check Local Path
        std::string final_path = filename;
        std::ifstream file(final_path);

        // 2. Check Global Library Path (~/.cipr/libs/)
        if (!file.is_open()) {
            if (const char* home = std::getenv("HOME")) {
                final_path = std::string(home) + "/.cipr/libs/" + filename;
                file.open(final_path);
            }
        }

        if (!file.is_open())
            return false;

        std::stringstream buf;
        buf << file.rdbuf();
        
        Scanner scanner(buf.str());
        auto tokens = scanner.scanTokens();
        Parser parser(tokens, interpreter.getArena());
        int root = parser.parse();
        interpreter.interpret(root);
        return true;
    }

    std::string toString() override {
        return "<native fn include>";
    }
};

struct NativeRand final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        if (!std::holds_alternative<double>(args[0]))
            return 0.0;

        const int max = static_cast<int>(std::get<double>(args[0]));

        if (max <= 0)
            return 0.0;
        
        static std::random_device rd;
        static std::mt19937 gen(rd());
        std::uniform_int_distribution<> dis(0, max - 1);
        
        return static_cast<double>(dis(gen));
    }

    std::string toString() override {
        return "<native fn rand>";
    }
};

struct NativeSleep final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        if (!std::holds_alternative<double>(args[0]))
            return std::monostate{};
        const int ms = static_cast<int>(std::get<double>(args[0]));
        std::this_thread::sleep_for(std::chrono::milliseconds(ms));

        return std::monostate{};
    }

        std::string toString() override {
        return "<native fn sleep>";
    }
};

struct NativeExit final : Callable {
    int arity() override {
        return 1;
    }

    Literal call(Interpreter&, const std::vector<Literal> args) override {
        int code = 0;

        if (std::holds_alternative<double>(args[0])) {
            code = static_cast<int>(std::get<double>(args[0]));
        }
        std::exit(code);
    }

    std::string toString() override {
        return "<native fn exit>";
    }
};

#endif