# Ori Playground

Browser-based playground for the Ori programming language. Runs the interpreter entirely in WebAssembly.

This standalone playground uses the **same Svelte component** as the website (`website/src/components/playground/`).

## Quick Start

### 1. Install wasm-pack

```bash
cargo install wasm-pack
```

### 2. Build the WASM module

```bash
cd playground
bun run build:wasm:all
```

This builds the WASM and copies it to both `playground/pkg/` and `website/src/wasm/`.

### 3. Install dependencies

```bash
cd playground
bun install
```

### 4. Start the dev server

```bash
bun run dev
```

### 5. Open in browser

Navigate to `http://localhost:3000`

## Project Structure

```
playground/
├── package.json       # Vite + Svelte project config
├── vite.config.ts     # Vite configuration
├── svelte.config.js   # Svelte 5 configuration
├── index.html         # Entry HTML
├── src/
│   ├── main.ts        # Mounts the Playground component
│   ├── components/    # Symlink → website/src/components/playground
│   └── wasm/          # Symlink → website/src/wasm
├── pkg/               # Generated WASM output (after build)
└── wasm/              # WASM crate source
    ├── Cargo.toml
    └── src/
        └── lib.rs     # Rust → WASM bindings
```

## Scripts

| Command | Description |
|---------|-------------|
| `bun run dev` | Start Vite development server |
| `bun run build` | Build for production |
| `bun run build:wasm` | Rebuild the WASM module |
| `bun run build:wasm:all` | Rebuild WASM and copy to website |

## Features

- **Monaco Editor** with Ori syntax highlighting
- **Format button** - formats code using `ori_fmt`
- **Auto-format on Run** - code is formatted before execution
- **Examples** dropdown with sample programs
- **Share** button (encodes code in URL hash)
- **Ctrl+Enter** to run

## Shared Component

The Playground Svelte component is shared between:
- `website/` - The main Ori website (Astro)
- `playground/` - This standalone development playground (Vite)

Both import from `website/src/components/playground/` via symlinks.
