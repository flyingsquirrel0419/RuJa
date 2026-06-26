# Changelog

## [0.1.0] - 2026-06-26

Initial release of RuJa, a JavaScript engine written in Rust.

### Added
- Lexer with full token coverage, ASI, and comment support
- Pratt-style recursive-descent parser producing an AST
- Tree-walking interpreter with closures and prototype chains
- `var`/`let`/`const` scoping, block and function scope
- Control flow: `if`/`else`, `while`, `do...while`, `for`, `for...in`, `for...of`,
  `switch`, labeled `break`/`continue`
- Functions, closures, and arrow functions with lexical `this`
- Objects, arrays, prototype chain, `new`, `instanceof`, `this` binding
- `throw`/`try`/`catch`/`finally` with `Error` type hierarchy
- Built-in objects: `Object`, `Array`, `String`, `Number`, `Boolean`,
  `Function`, `Math`, `JSON`, `console`
- Array methods: `push`, `pop`, `shift`, `unshift`, `join`, `slice`, `concat`,
  `reverse`, `forEach`, `map`, `filter`, `reduce`, `find`, `includes`, `indexOf`
- String methods: `charAt`, `charCodeAt`, `slice`, `substring`, `substr`,
  `split`, `replace`, `includes`, `startsWith`, `endsWith`, `repeat`, `trim`,
  `toUpperCase`, `toLowerCase`, `concat`
- `parseInt`, `parseFloat`, `isNaN`, `isFinite`
- Spread in arrays and calls, nullish coalescing
- CLI with file execution, `-e` eval, `--version`, `--help`
- Interactive REPL with multi-line block support
- JSON parse and stringify
