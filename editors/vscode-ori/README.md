# Ori Language Support for VS Code

Syntax highlighting, language configuration, and snippets for the [Ori programming language](https://github.com/upstat-io/ori-lang).

The TextMate grammar follows the authoritative spec grammar at `docs/ori_lang/v2026/spec/grammar.ebnf`.

## Features

- Syntax highlighting for `.ori` files (full lexical surface: template strings with interpolation, char/byte/duration/size literals, attributes, FFI blocks, labels, contracts, capabilities)
- Bracket matching, auto-closing, and colorized bracket pairs
- Comment toggling (`Ctrl+/` or `Cmd+/`)
- Indentation rules for expression-bodied declarations (`@f () -> int =` indents the next line)
- Snippets for common declaration shapes (`fn`, `type`, `trait`, `impl`, `match`, `test`, ...)

## Installation

Modern VS Code (1.74+) only loads extensions registered in its `extensions.json` registry — dropping or symlinking a folder into `~/.vscode/extensions/` no longer works. Install via a packaged `.vsix`.

### Option 1: VSIX install (Recommended)

```bash
cd <repo-root>/editors/vscode-ori
npm install
npm run install-extension
```

This packages the extension with `vsce` and installs it via `code --install-extension`. Under WSL2 the `code` CLI installs into the Remote-WSL server (`~/.vscode-server/extensions/`) automatically.

Then run "Developer: Reload Window" from the command palette (or restart VS Code).

To update after grammar changes: re-run `npm run install-extension` and reload the window.

### Option 2: Debug Mode (F5)

1. Open the `editors/vscode-ori` folder in VS Code
2. Press `F5` to launch an Extension Development Host window with the extension loaded
3. Open any `.ori` file in the new window

## Syntax Highlighting

| Element | Example | Scope |
|---------|---------|-------|
| Functions | `@fibonacci`, `@main` | `entity.name.function.ori` |
| Constants / const generics | `let $timeout`, `<$N: int>` | `variable.other.constant.ori` |
| Keywords | `if`, `then`, `else`, `match`, `let`, `type`, `impl`, `while`, `yield` | `keyword.control.ori` / `keyword.declaration.ori` |
| Modifiers | `pub`, `suspend`, `unsafe`, `extern` | `storage.modifier.ori` |
| Pattern expressions | `recurse(`, `parallel(`, `timeout(`, `try {` | `keyword.control.pattern.ori` |
| Built-in functions | `print(`, `len(`, `panic(`, `embed(` | `support.function.builtin.ori` |
| Primitive types | `int`, `str`, `bool`, `Never` | `support.type.primitive.ori` |
| Built-in types | `Option`, `Result`, `Duration`, `CPtr` | `support.type.builtin.ori` |
| Variants | `Ok`, `Err`, `Some`, `None`, `Less` | `support.constant.variant.ori` |
| User types | `Point`, `UserId` | `entity.name.type.ori` |
| Named args / fields | `f(over: xs)`, punning `f(x:)` | `variable.parameter.ori` |
| Strings + escapes | `"hi\n"`, `\u{1F600}`, `\xFF` | `string.quoted.double.ori` |
| Template strings | `` `value: {x:>10.2f}` `` | `string.interpolated.ori` |
| Char / byte literals | `'a'`, `'\x41'`, `b'x'`, `b'\xFF'` | `string.quoted.single.{char,byte}.ori` |
| Numbers | `1_000`, `0xFF`, `0b1010`, `2.5e-8` | `constant.numeric.*.ori` |
| Duration / Size | `100ms`, `0.5s`, `1.5kb`, `10tb` | `constant.numeric.{duration,size}.ori` |
| Doc comments | `// * field:`, `// ! Error:`, `// > example` | `comment.line.documentation.*.ori` |
| Attributes | `#derive(Eq)`, `#skip("...")`, `#!target(...)` | `meta.attribute.ori` |
| Labels | `loop:outer`, `break:outer value` | `entity.name.label.ori` |
| Contracts | `pre(x > 0)`, `post(r -> r >= 0)` | `keyword.other.contract.ori` |
| Capabilities | `uses Http, Clock` | `keyword.other.capability.ori` |
| FFI | `extern "c" from "lib" { ... }`, `out`/`owned`/`borrowed` | `meta.extern.ori` |
| Future-reserved words | `asm`, `inline`, `static`, `union`, `view` | `invalid.deprecated.reserved.ori` |

Words that are NOT Ori keywords render as plain identifiers by design: `return`, `async`, `await`, `fn`, `nil` (Ori has no `return` — the last expression is the block value).

## Troubleshooting

**Extension not loading?**

- Verify it is registered: `code --list-extensions | grep ori-lang` (run inside WSL for Remote-WSL windows)
- Check VS Code's extension host log: Help > Toggle Developer Tools > Console

**Syntax not highlighting?**

- Verify the file has the `.ori` extension
- Check the language mode in the status bar (should say "Ori")
- Try "Developer: Reload Window"

## Development

1. Edit `syntaxes/ori.tmLanguage.json` (keep it conformant to `docs/ori_lang/v2026/spec/grammar.ebnf` — the spec is the source of truth)
2. Run the tokenization tests: `npm install && npm test` (asserts scopes with `vscode-textmate` + `vscode-oniguruma`, the engine VS Code runs)
3. Dump scopes for any file: `node test/tokenize.js path/to/file.ori`
4. Reload the VS Code window and eyeball the conformance corpus in `tests/spec/` (`tests/spec/lexical/` exercises every literal form)

Use "Developer: Inspect Editor Tokens and Scopes" to debug token scopes interactively.
