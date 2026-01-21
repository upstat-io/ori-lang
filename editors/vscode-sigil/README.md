# Sigil Language Support for VS Code

Syntax highlighting for the [Sigil programming language](https://github.com/sigil-lang/sigil).

## Features

- Syntax highlighting for `.si` files
- Bracket matching and auto-closing
- Comment toggling (`Ctrl+/` or `Cmd+/`)

## Installation (Development Mode)

Since this extension is in-repo, you can load it directly in VS Code without publishing:

### Option 1: Symbolic Link (Recommended for Development)

1. Find your VS Code extensions directory:
   - **Linux**: `~/.vscode/extensions/`
   - **macOS**: `~/.vscode/extensions/`
   - **Windows**: `%USERPROFILE%\.vscode\extensions\`

2. Create a symbolic link:
   ```bash
   # Linux/macOS
   ln -s /home/eric/sigil_lang/editors/vscode-sigil ~/.vscode/extensions/sigil-lang

   # Windows (PowerShell as Admin)
   New-Item -ItemType SymbolicLink -Path "$env:USERPROFILE\.vscode\extensions\sigil-lang" -Target "C:\path\to\sigil_lang\editors\vscode-sigil"
   ```

3. Restart VS Code or run "Developer: Reload Window" from the command palette.

### Option 2: Debug Mode (F5)

1. Open the `editors/vscode-sigil` folder in VS Code
2. Press `F5` to launch a new VS Code window with the extension loaded
3. Open any `.si` file in the new window to see syntax highlighting

### Option 3: Copy Folder

Copy the entire `editors/vscode-sigil` folder to your extensions directory:
```bash
cp -r /home/eric/sigil_lang/editors/vscode-sigil ~/.vscode/extensions/sigil-lang
```

## Syntax Highlighting

The extension highlights:

| Element | Example | Color Category |
|---------|---------|----------------|
| Functions | `@fibonacci` | Function |
| Config vars | `$timeout` | Constant |
| Keywords | `if`, `let`, `type`, `impl` | Keyword |
| Patterns | `map(`, `filter(`, `fold(` | Support Function |
| Types | `int`, `str`, `Result` | Type |
| Named args | `.over:`, `.transform:` | Parameter |
| Result/Option | `Ok`, `Err`, `Some`, `None` | Constant |
| Strings | `"hello"` | String |
| Numbers | `42`, `3.14`, `30s` | Number |
| Comments | `// comment` | Comment |
| Attributes | `#[derive(Eq)]` | Attribute |

## Updating

Since this is symlinked, any changes to the grammar files will take effect after reloading VS Code ("Developer: Reload Window").

## Troubleshooting

**Extension not loading?**
- Ensure the symlink is correct: `ls -la ~/.vscode/extensions/sigil-lang`
- Check VS Code's extension host log: Help > Toggle Developer Tools > Console

**Syntax not highlighting?**
- Verify the file has `.si` extension
- Check the language mode in the status bar (should say "Sigil")
- Try "Developer: Reload Window"

## Development

To modify the syntax highlighting:

1. Edit `syntaxes/sigil.tmLanguage.json`
2. Reload VS Code window
3. Test with sample `.si` files in `tests/run-pass/`

Use VS Code's "Developer: Inspect Editor Tokens and Scopes" to debug highlighting issues.
