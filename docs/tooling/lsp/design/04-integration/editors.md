---
title: "Editor Integration"
description: "Ori LSP Design — VS Code and Neovim Integration"
order: 3
section: "Integration"
---

# Editor Integration

> **Status: Not Yet Implemented.** The editor integrations described below are planned but not yet available. No VS Code extension, Neovim plugin, or other editor packages have been published.

Integrating the native LSP server with desktop editors.

## VS Code Extension

### Extension Structure

```
ori-vscode/
├── package.json            # Extension manifest
├── src/
│   └── extension.ts        # Extension entry point
├── bin/
│   ├── ori_lsp-linux       # Linux binary
│   ├── ori_lsp-macos       # macOS binary
│   └── ori_lsp-windows.exe # Windows binary
├── syntaxes/
│   └── ori.tmLanguage.json # TextMate grammar
└── language-configuration.json
```

### package.json

```json
{
  "name": "ori-lang",
  "displayName": "Ori Language",
  "description": "Ori language support for VS Code",
  "version": "0.1.0",
  "publisher": "ori-lang",
  "engines": {
    "vscode": "^1.75.0"
  },
  "categories": ["Programming Languages"],
  "activationEvents": [
    "onLanguage:ori"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [{
      "id": "ori",
      "aliases": ["Ori", "ori"],
      "extensions": [".ori"],
      "configuration": "./language-configuration.json"
    }],
    "grammars": [{
      "language": "ori",
      "scopeName": "source.ori",
      "path": "./syntaxes/ori.tmLanguage.json"
    }],
    "configuration": {
      "title": "Ori",
      "properties": {
        "ori.server.path": {
          "type": "string",
          "default": "",
          "description": "Path to ori_lsp binary (uses bundled if empty)"
        },
        "ori.trace.server": {
          "type": "string",
          "enum": ["off", "messages", "verbose"],
          "default": "off",
          "description": "Trace communication with the language server"
        }
      }
    }
  },
  "dependencies": {
    "vscode-languageclient": "^9.0.0"
  }
}
```

### Extension Entry Point

```typescript
// src/extension.ts
import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    // Find server binary
    const serverPath = getServerPath(context);

    // Server options: spawn the binary
    const serverOptions: ServerOptions = {
        run: { command: serverPath },
        debug: { command: serverPath, args: ['--debug'] },
    };

    // Client options
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'ori' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.ori'),
        },
    };

    // Create and start client
    client = new LanguageClient(
        'ori',
        'Ori Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}

function getServerPath(context: ExtensionContext): string {
    // Check user configuration
    const config = workspace.getConfiguration('ori');
    const configPath = config.get<string>('server.path');
    if (configPath) {
        return configPath;
    }

    // Use bundled binary
    const platform = process.platform;
    const binary = platform === 'win32' ? 'ori_lsp.exe' : 'ori_lsp';
    const platformDir = platform === 'darwin' ? 'macos' : platform;

    return path.join(
        context.extensionPath,
        'bin',
        platformDir,
        binary
    );
}
```

### Language Configuration

```json
// language-configuration.json
{
  "comments": {
    "lineComment": "//"
  },
  "brackets": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"]
  ],
  "autoClosingPairs": [
    { "open": "{", "close": "}" },
    { "open": "[", "close": "]" },
    { "open": "(", "close": ")" },
    { "open": "\"", "close": "\"" },
    { "open": "`", "close": "`" }
  ],
  "surroundingPairs": [
    { "open": "{", "close": "}" },
    { "open": "[", "close": "]" },
    { "open": "(", "close": ")" },
    { "open": "\"", "close": "\"" },
    { "open": "`", "close": "`" }
  ],
  "indentationRules": {
    "increaseIndentPattern": "^.*\\{[^}]*$|^.*\\([^)]*$|^.*\\[[^\\]]*$",
    "decreaseIndentPattern": "^\\s*[}\\])]"
  }
}
```

### Building and Publishing

```bash
# Build extension
npm run compile

# Package for distribution
npx vsce package

# Publish to marketplace
npx vsce publish
```

## Neovim Configuration

### Native LSP Setup

Neovim 0.5+ has built-in LSP support:

```lua
-- ~/.config/nvim/lua/lsp/ori.lua

local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

-- Define the Ori language server
if not configs.ori_lsp then
    configs.ori_lsp = {
        default_config = {
            cmd = { 'ori_lsp' },  -- Must be in PATH
            filetypes = { 'ori' },
            root_dir = function(fname)
                return lspconfig.util.find_git_ancestor(fname)
                    or vim.fn.getcwd()
            end,
            settings = {},
        },
    }
end

-- Set up the server
lspconfig.ori_lsp.setup({
    on_attach = function(client, bufnr)
        -- Enable completion
        vim.api.nvim_buf_set_option(bufnr, 'omnifunc', 'v:lua.vim.lsp.omnifunc')

        -- Keybindings
        local opts = { noremap = true, silent = true, buffer = bufnr }
        vim.keymap.set('n', 'K', vim.lsp.buf.hover, opts)
        vim.keymap.set('n', 'gd', vim.lsp.buf.definition, opts)
        vim.keymap.set('n', 'gr', vim.lsp.buf.references, opts)
        vim.keymap.set('n', '<leader>f', vim.lsp.buf.format, opts)
        vim.keymap.set('n', '<leader>rn', vim.lsp.buf.rename, opts)
    end,
    capabilities = require('cmp_nvim_lsp').default_capabilities(),
})
```

### Filetype Detection

```lua
-- ~/.config/nvim/ftdetect/ori.lua
vim.filetype.add({
    extension = {
        ori = 'ori',
    },
})
```

### Syntax Highlighting

Until tree-sitter support is added:

```vim
" ~/.config/nvim/syntax/ori.vim
if exists("b:current_syntax")
    finish
endif

" Keywords
syn keyword oriKeyword let pub type trait impl use uses
syn keyword oriKeyword if then else match for in do yield
syn keyword oriKeyword true false void self Self
syn keyword oriKeyword run try catch parallel spawn timeout cache with

" Functions (@ prefix)
syn match oriFunction "@\w\+"

" Constants ($ prefix)
syn match oriConstant "\$\w\+"

" Types (capitalized)
syn match oriType "\<[A-Z][A-Za-z0-9_]*\>"

" Comments
syn match oriComment "//.*$"

" Strings
syn region oriString start='"' end='"' skip='\\"'
syn region oriTemplate start='`' end='`' skip='\\`'

" Numbers
syn match oriNumber "\<\d\+\>"
syn match oriNumber "\<\d\+\.\d\+\>"

" Operators
syn match oriOperator "[-+*/%=<>!&|^~?:]"
syn match oriOperator "->"
syn match oriOperator "=>"

" Highlighting
hi def link oriKeyword Keyword
hi def link oriFunction Function
hi def link oriConstant Constant
hi def link oriType Type
hi def link oriComment Comment
hi def link oriString String
hi def link oriTemplate String
hi def link oriNumber Number
hi def link oriOperator Operator

let b:current_syntax = "ori"
```

### Format on Save

```lua
-- Auto-format on save
vim.api.nvim_create_autocmd("BufWritePre", {
    pattern = "*.ori",
    callback = function()
        vim.lsp.buf.format({ async = false })
    end,
})
```

## Binary Distribution

### Build Script

```bash
#!/bin/bash
# scripts/build-lsp-binaries.sh

set -e

# Build for current platform
cargo build --release -p ori_lsp

# Cross-compile (requires cross-compilation setup)
if command -v cross &> /dev/null; then
    cross build --release -p ori_lsp --target x86_64-unknown-linux-gnu
    cross build --release -p ori_lsp --target x86_64-apple-darwin
    cross build --release -p ori_lsp --target x86_64-pc-windows-gnu
fi

# Copy to distribution folder
mkdir -p dist/bin
cp target/release/ori_lsp dist/bin/ori_lsp-$(uname -s | tr '[:upper:]' '[:lower:]')
```

### Release Automation

GitHub Actions workflow:

```yaml
# .github/workflows/release-lsp.yml
name: Release LSP

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: ori_lsp-linux
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: ori_lsp-macos
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: ori_lsp-windows.exe

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Build
        run: cargo build --release -p ori_lsp --target ${{ matrix.target }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/ori_lsp*
```

## Installation Instructions

### VS Code

1. Install from marketplace: `ext install ori-lang.ori-lang`
2. Or: Download `.vsix` and install manually

### Neovim

1. Install `ori_lsp` binary:
   ```bash
   # Via cargo
   cargo install ori_lsp

   # Or download from releases
   curl -L https://github.com/ori-lang/ori/releases/latest/download/ori_lsp-linux -o ~/.local/bin/ori_lsp
   chmod +x ~/.local/bin/ori_lsp
   ```

2. Add LSP configuration (see above)

### Other Editors

Any LSP-compatible editor can use `ori_lsp`:

| Editor | LSP Plugin |
|--------|------------|
| Emacs | `lsp-mode` or `eglot` |
| Sublime Text | `LSP` package |
| Helix | Built-in |
| Zed | Built-in |

Generic configuration:
- Command: `ori_lsp`
- File pattern: `*.ori`
- Language ID: `ori`

## Debugging

### VS Code

Enable tracing in settings:

```json
{
    "ori.trace.server": "verbose"
}
```

View output in: Output → Ori Language Server

### Neovim

```lua
-- Enable LSP logging
vim.lsp.set_log_level("debug")

-- View logs
:lua vim.cmd('edit ' .. vim.lsp.get_log_path())
```

### Manual Testing

Run server directly:

```bash
# Start server in stdio mode
ori_lsp

# Send initialize request (copy-paste JSON)
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}

# Server responds with capabilities
```
