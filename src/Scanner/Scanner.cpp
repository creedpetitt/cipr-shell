#include "Scanner.h"
#include <map>
#include <utility>
#include "Core/Core.h"

Scanner::Scanner(std::string source) : source_(std::move(source)) {}

std::vector<Token> Scanner::scanTokens() {
    while (!isAtEnd()) {
        start = current;
        scanToken();
    }

    tokens.emplace_back(EOF_TOKEN, "", std::monostate{}, line);
    return tokens;
}

void Scanner::scanToken() {
    const char c = advance();
    switch (c) {
        case '(': addToken(LEFT_PAREN); break;
        case ')': addToken(RIGHT_PAREN); break;
        case '{': addToken(LEFT_BRACE); break;
        case '}': addToken(RIGHT_BRACE); break;
        case '[': addToken(LEFT_BRACKET); break;
        case ']': addToken(RIGHT_BRACKET); break;
        case '$': addToken(DOLLAR); break;
        case ',': addToken(COMMA); break;
        case '.': addToken(DOT); break;
        case '-': addToken(MINUS); break;
        case '+': addToken(PLUS); break;
        case ';': addToken(SEMICOLON); break;
        case '*': addToken(STAR); break;

        case '!':
            addToken(match('=') ? BANG_EQUAL : BANG);
            break;
        case '=':
            addToken(match('=') ? EQUAL_EQUAL : EQUAL);
            break;
        case '<':
            addToken(match('=') ? LESS_EQUAL : LESS);
            break;
        case '>':
            addToken(match('=') ? GREATER_EQUAL : GREATER);
            break;

        case '/':
            if (match('/')) {
                // A comment goes until the end of the line.
                while (peek() != '\n' && !isAtEnd())
                    advance();
            } else {
                addToken(SLASH);
            }
            break;

        case '"': string('"'); break;
        case '\'': string('\''); break;

        case ' ':
        case '\r':
        case '\t':
            // Ignore whitespace.
            break;

        case '\n':
            line++;
            break;

        default:
            if (isDigit(c)) {
                number();
            } else if (isAlpha(c)) {
                identifier();
            } else {
                Core::error(line, "Unexpected character.");
            }
            break;
    }
}

bool Scanner::isAtEnd() const {
    return current >= source_.length();
}

char Scanner::advance()  {
    return source_[current++];
}

bool Scanner::match(const char expected) {
    if (isAtEnd())
        return false;
    if (source_[current] != expected)
        return false;

    current++; // Only move forward if it's the character expected
    return true;
}

char Scanner::peek() const {
    if (isAtEnd()) return '\0';
    return source_[current];
}

char Scanner::peekNext() const {
    if (current + 1 >= source_.length())
        return '\0';
    return source_[current + 1];
}

void Scanner::identifier() {
    while (isAlphaNumeric(peek()))
        advance();

    const std::string text = source_.substr(start, current - start);
    static const std::map<std::string, TokenType> keywords = {
        {"and", AND},
        {"class", CLASS},
        {"else", ELSE},
        {"false", FALSE},
        {"fn", FN},
        {"for", FOR},
        {"if", IF},
        {"null", TOK_NULL},
        {"or", OR},
        {"echo", ECHO},
        {"return", RETURN},
        {"super", SUPER},
        {"this", THIS},
        {"true", TRUE},
        {"let", LET},
        {"while", WHILE}
    };

    const auto it = keywords.find(text);
    TokenType type = it != keywords.end() ? it->second : IDENTIFIER;
    addToken(type);
}

void Scanner::string(char delimiter) {
    std::string value;
    while (peek() != delimiter && !isAtEnd()) {
        if (peek() == '\n')
            line++;
        
        if (peek() == '\\') {
            advance(); // consume '\'
            if (isAtEnd()) break;
            switch (peek()) {
                case 'n': value += '\n'; break;
                case 't': value += '\t'; break;
                case 'r': value += '\r'; break;
                case '\\': value += '\\'; break;
                case '"': value += '"'; break;
                case '\'': value += '\''; break;
                default: 
                    value += '\\';
                    value += peek();
                    break;
            }
        } else {
            value += peek();
        }
        advance();
    }

    if (isAtEnd()) {
        Core::error(line, "Unterminated string.");
        return;
    }

    advance(); // The closing delimiter
    addToken(STRING, value);
}

void Scanner::number() {
    while (isDigit(peek()))
        advance();

    if (peek() == '.' && isDigit(peekNext())) {
        advance();

        while (isDigit(peek()))
            advance();
    }

    addToken(NUMBER, std::stod(source_.substr(start, current - start)));
}

bool Scanner::isDigit(const char c) {
    return c >= '0' && c <= '9';
}

bool Scanner::isAlpha(const char c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}

bool Scanner::isAlphaNumeric(const char c) {
    return isAlpha(c) || isDigit(c);
}

void Scanner::addToken(const TokenType type) {
    addToken(type, std::monostate{});
}

void Scanner::addToken(const TokenType type, const Literal& literal) {
    const std::string text = source_.substr(start, current - start);
    tokens.emplace_back(type, text, literal, line);
}