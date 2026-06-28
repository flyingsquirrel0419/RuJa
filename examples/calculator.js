// A small expression calculator with operator precedence.
//
// Grammar:
//   expr     := term (("+" | "-") term)*
//   term     := factor (("*" | "/") factor)*
//   factor   := number | "(" expr ")"
//
// Run:  cargo run --release -- examples/calculator.js
// REPL: ./target/release/ruja

const PRECEDENCE = { "+": 1, "-": 1, "*": 2, "/": 2 };

// Tokenize a raw expression into numbers and operators.
function tokenize(src) {
    const tokens = [];
    let i = 0;
    while (i < src.length) {
        let ch = src[i];
        // skip whitespace
        if (ch === " " || ch === "\t" || ch === "\n") { i++; continue; }
        // number (int or float)
        if ((ch >= "0" && ch <= "9") || ch === ".") {
            let start = i;
            while (i < src.length && ((src[i] >= "0" && src[i] <= "9") || src[i] === ".")) i++;
            tokens.push({ type: "num", value: parseFloat(src.slice(start, i)) });
            continue;
        }
        // operator or paren
        if (ch === "+" || ch === "-" || ch === "*" || ch === "/" ||
            ch === "(" || ch === ")") {
            tokens.push({ type: "op", value: ch });
            i++;
            continue;
        }
        throw new Error("Unexpected character: " + ch);
    }
    return tokens;
}

// Recursive-descent parser. peek()/next() walk the token stream.
function Parser(tokens) {
    let pos = 0;
    function peek() { return tokens[pos]; }
    function next() { return tokens[pos++]; }
    function expectOp(op) {
        let t = next();
        if (!t || t.type !== "op" || t.value !== op) {
            throw new Error("Expected '" + op + "'");
        }
    }

    function parseExpr() {
        let left = parseTerm();
        while (peek() && peek().type === "op" &&
               (peek().value === "+" || peek().value === "-")) {
            let op = next().value;
            let right = parseTerm();
            left = op === "+" ? left + right : left - right;
        }
        return left;
    }

    function parseTerm() {
        let left = parseFactor();
        while (peek() && peek().type === "op" &&
               (peek().value === "*" || peek().value === "/")) {
            let op = next().value;
            let right = parseFactor();
            if (op === "/" && right === 0) {
                throw new Error("Division by zero");
            }
            left = op === "*" ? left * right : left / right;
        }
        return left;
    }

    function parseFactor() {
        let t = next();
        if (!t) throw new Error("Unexpected end of input");
        if (t.type === "num") return t.value;
        if (t.type === "op" && t.value === "(") {
            let val = parseExpr();
            expectOp(")");
            return val;
        }
        throw new Error("Unexpected token: " + t.value);
    }

    return { parse: parseExpr };
}

// Evaluate a single expression string.
function evaluate(src) {
    const tokens = tokenize(src);
    if (tokens.length === 0) return 0;
    return Parser(tokens).parse();
}

// --- Demo ---

const cases = [
    "1 + 2",
    "3 + 5 * 2",
    "(3 + 5) * 2",
    "10 - 8 / 4",
    "2 * (3 + 4) - 5",
    "100 / 4 / 5",
    "3.5 * 2 + 1",
];

console.log("=== RuJa Calculator ===\n");
for (let i = 0; i < cases.length; i++) {
    let c = cases[i];
    let result = evaluate(c);
    let pad = c;
    while (pad.length < 16) pad = pad + " ";
    console.log(pad + " = " + result);
}

// Error handling: division by zero and bad tokens.
console.log("\n=== Error cases ===");
const errors = ["10 / 0", "3 + abc", "3 + / 2"];
for (let i = 0; i < errors.length; i++) {
    try {
        evaluate(errors[i]);
        console.log(errors[i] + "  -> (no error?!)");
    } catch (e) {
        console.log(errors[i] + "  -> " + e.message);
    }
}
