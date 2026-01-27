# Ori Playground

Browser-based playground for the Ori programming language. Runs the interpreter entirely in WebAssembly.

## Quick Start

### 1. Install wasm-pack

```bash
cargo install wasm-pack
```

### 2. Build the WASM module

```bash
cd playground
bun run build:wasm
```

Or manually:

```bash
cd playground/wasm
wasm-pack build --target web --out-dir ../pkg
```

### 3. Start the dev server

```bash
cd playground
bun run dev
```

### 4. Open in browser

Navigate to `http://localhost:3000`

## Project Structure

```
playground/
├── package.json    # Bun project config
├── server.ts       # Bun dev server
├── index.html      # Main page
├── style.css       # Styling (VS Code dark theme)
├── app.js          # Monaco editor + WASM integration
├── pkg/            # Generated WASM output (after build)
└── wasm/           # WASM crate
    ├── Cargo.toml
    └── src/
        └── lib.rs  # Rust → WASM bindings via wasm-bindgen
```

## Scripts

| Command | Description |
|---------|-------------|
| `bun run dev` | Start development server on port 3000 |
| `bun run build:wasm` | Rebuild the WASM module |

## Features

- **Monaco Editor** with Ori syntax highlighting
- **VS Code Dark+ theme**
- **Examples** dropdown with sample programs
- **Share** button (encodes code in URL hash)
- **Ctrl+Enter** to run

## How It Works

The playground uses wasm-bindgen to expose the Ori interpreter to JavaScript:

1. `lib.rs` exports `run_ori(source: &str) -> String` which returns JSON
2. The WASM module uses the same Salsa-based interpreter as `cargo st`
3. `app.js` imports the WASM module and calls `run_ori()` when you click Run

## Deployment

The playground is static files only. Deploy to GitHub Pages, Cloudflare Pages, Netlify, Vercel, or any static host.

Just copy the `playground/` directory (after building WASM).
