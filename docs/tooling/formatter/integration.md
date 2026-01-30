# Integration Guide: Editor Setup

This guide covers setting up automatic formatting in editors and IDEs. Format-on-save keeps code consistently styled without manual intervention.

## Quick Setup

Most editors can use `ori fmt --stdin` for format-on-save. The formatter reads from stdin and writes formatted output to stdout.

```bash
# Test it works
echo '@main()->void=print(msg:"Hello")' | ori fmt --stdin
# Output: @main () -> void = print(msg: "Hello")
```

## VS Code

### Using Run on Save Extension

1. Install the "Run on Save" extension (`emeraldwalk.RunOnSave`)

2. Add to `.vscode/settings.json`:

```json
{
    "emeraldwalk.runonsave": {
        "commands": [
            {
                "match": ".*\\.ori$",
                "cmd": "ori fmt ${file}"
            }
        ]
    }
}
```

### Using Custom Task

1. Create `.vscode/tasks.json`:

```json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "Format Ori",
            "type": "shell",
            "command": "ori fmt ${file}",
            "problemMatcher": [],
            "presentation": {
                "reveal": "silent"
            }
        }
    ]
}
```

2. Bind to a keyboard shortcut in `keybindings.json`:

```json
{
    "key": "ctrl+shift+f",
    "command": "workbench.action.tasks.runTask",
    "args": "Format Ori",
    "when": "editorLangId == ori"
}
```

### Future: Native LSP Support

When `ori_lsp` is available, native format-on-save will be supported:

```json
{
    "[ori]": {
        "editor.formatOnSave": true
    }
}
```

## Neovim

### Using Vim Commands

Add to `~/.config/nvim/init.lua`:

```lua
-- Format Ori files on save
vim.api.nvim_create_autocmd("BufWritePre", {
    pattern = "*.ori",
    callback = function()
        -- Save cursor position
        local pos = vim.api.nvim_win_get_cursor(0)

        -- Format entire buffer
        vim.cmd('%!ori fmt --stdin')

        -- Restore cursor position
        local lines = vim.api.nvim_buf_line_count(0)
        pos[1] = math.min(pos[1], lines)
        vim.api.nvim_win_set_cursor(0, pos)
    end,
})

-- Manual format command
vim.api.nvim_create_user_command('OriFmt', function()
    vim.cmd('%!ori fmt --stdin')
end, {})
```

### Using conform.nvim

If you use [conform.nvim](https://github.com/stevearc/conform.nvim) for formatting:

```lua
require('conform').setup({
    formatters_by_ft = {
        ori = { 'ori_fmt' },
    },
    formatters = {
        ori_fmt = {
            command = 'ori',
            args = { 'fmt', '--stdin' },
            stdin = true,
        },
    },
    format_on_save = {
        timeout_ms = 500,
        lsp_fallback = false,
    },
})
```

### Future: Native LSP Support

When `ori_lsp` is available:

```lua
-- Format on save via LSP
vim.api.nvim_create_autocmd("BufWritePre", {
    pattern = "*.ori",
    callback = function()
        vim.lsp.buf.format({ async = false })
    end,
})
```

## Emacs

### Using Format-All

Add to your config:

```elisp
;; Define ori-mode formatter
(with-eval-after-load 'format-all
  (define-format-all-formatter ori-fmt
    (:executable "ori")
    (:install "cargo install ori")
    (:languages "Ori")
    (:format (format-all--buffer-easy executable "fmt" "--stdin"))))

;; Enable format-on-save for Ori files
(add-hook 'ori-mode-hook #'format-all-mode)
```

### Using Reformatter

If you use [reformatter.el](https://github.com/purcell/reformatter.el):

```elisp
(use-package reformatter
  :config
  (reformatter-define ori-format
    :program "ori"
    :args '("fmt" "--stdin")))

;; Enable on save
(add-hook 'ori-mode-hook #'ori-format-on-save-mode)
```

## Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "ori"
scope = "source.ori"
file-types = ["ori"]
comment-token = "//"
indent = { tab-width = 4, unit = "    " }
formatter = { command = "ori", args = ["fmt", "--stdin"] }
auto-format = true
```

## Sublime Text

### Using Format on Save Package

1. Install "Format on Save" via Package Control

2. Create `Packages/User/format_on_save.sublime-settings`:

```json
{
    "formatters": [
        {
            "name": "ori fmt",
            "selector": "source.ori",
            "cmd": ["ori", "fmt", "--stdin"]
        }
    ]
}
```

### Using Build System

Create `Packages/User/OriFormat.sublime-build`:

```json
{
    "cmd": ["ori", "fmt", "$file"],
    "selector": "source.ori"
}
```

## JetBrains IDEs (IntelliJ, CLion, etc.)

### External Tool Setup

1. Go to **Settings → Tools → External Tools**
2. Add a new tool:
   - Name: `Ori Format`
   - Program: `ori`
   - Arguments: `fmt $FilePath$`
   - Working directory: `$ProjectFileDir$`

3. Optionally bind to keyboard shortcut:
   - **Settings → Keymap → External Tools → Ori Format**

### File Watcher Setup

1. Install the "File Watchers" plugin
2. Go to **Settings → Tools → File Watchers**
3. Add a new watcher:
   - File type: `Any`
   - Scope: `file:*.ori`
   - Program: `ori`
   - Arguments: `fmt $FilePath$`
   - Output paths: `$FilePath$`

## CI/CD Integration

### GitHub Actions

```yaml
name: Format Check

on: [push, pull_request]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Ori
        run: cargo install ori

      - name: Check formatting
        run: ori fmt --check
```

### GitLab CI

```yaml
format:
  stage: test
  script:
    - cargo install ori
    - ori fmt --check
```

### Pre-commit Hook

Create `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: ori-fmt
        name: ori fmt
        entry: ori fmt
        language: system
        files: '\.ori$'
        pass_filenames: true
```

Or use a simple git hook in `.git/hooks/pre-commit`:

```bash
#!/bin/bash
ori fmt --check || {
    echo "Run 'ori fmt' to fix formatting"
    exit 1
}
```

## Troubleshooting

### Command Not Found

Ensure `ori` is in your PATH:

```bash
# Check installation
which ori

# Add to PATH if needed (example for ~/.bashrc)
export PATH="$HOME/.cargo/bin:$PATH"
```

### Formatter Modifies File Unexpectedly

The formatter is deterministic — the same input always produces the same output. If files are being modified unexpectedly:

1. Check that your editor isn't adding/removing trailing newlines
2. Check that your editor uses the correct encoding (UTF-8)
3. Run `ori fmt --diff file.ori` to see exact changes

### Format Doesn't Work on Save

1. Check editor logs for errors
2. Test manually: `ori fmt path/to/file.ori`
3. Ensure the file is syntactically valid (formatter skips files with parse errors)

See [Troubleshooting Guide](troubleshooting.md) for more help.

## See Also

- [User Guide](user-guide.md) — Command-line usage
- [Style Guide](style-guide.md) — What the formatter enforces
- [LSP Editor Integration](../lsp/design/04-integration/editors.md) — Full LSP setup (when available)
