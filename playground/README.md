# Ori Playground

Browser-based playground for the Ori programming language. Runs the interpreter entirely in WebAssembly — no server required.

## Quick Start

### 1. Install wasm-pack

```bash
cargo install wasm-pack
```

### 2. Build the WASM module

```bash
cd playground/wasm
wasm-pack build --target web
```

This generates `playground/wasm/pkg/` with the JavaScript bindings.

### 3. Serve the playground

```bash
cd playground && bun --bun serve .
```

### 4. Open in browser

Navigate to `http://localhost:8080`

## Development

### Project Structure

```
playground/
├── index.html      # Main page
├── style.css       # Styling (VS Code dark theme)
├── app.js          # Monaco editor + WASM glue
├── wasm/           # WASM crate
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs  # Rust → WASM bindings
└── pkg/            # Generated WASM output (after build)
```

### Rebuilding

After making changes to the Rust code:

```bash
cd playground/wasm
wasm-pack build --target web
```

Then refresh the browser.

## Features

- **Monaco Editor** with Ori syntax highlighting
- **VS Code Dark+ theme**
- **Examples** dropdown with sample programs
- **Share** button (encodes code in URL hash)
- **Ctrl+Enter** to run

## Deployment

The playground is static files only. Deploy to:
- GitHub Pages
- Cloudflare Pages
- Netlify
- Vercel
- Any static host

Just copy the `playground/` directory (after building WASM).
