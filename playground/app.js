// Ori Playground Application

// Example programs
const EXAMPLES = {
    hello: `// Hello World in Ori
@main () -> void = print(msg: "Hello, World!")`,

    fibonacci: `// Fibonacci sequence
@fib (n: int) -> int =
    if n < 2
    then n
    else @fib(n: n - 1) + @fib(n: n - 2)

@main () -> void = run(
    print(msg: "fib(10) = " + str(@fib(n: 10)))
)

@test_fib tests @fib () -> void = run(
    assert_eq(actual: @fib(n: 0), expected: 0),
    assert_eq(actual: @fib(n: 1), expected: 1),
    assert_eq(actual: @fib(n: 10), expected: 55)
)`,

    factorial: `// Factorial with recursion
@factorial (n: int) -> int =
    if n <= 1
    then 1
    else n * @factorial(n: n - 1)

@main () -> void = run(
    print(msg: "5! = " + str(@factorial(n: 5)))
)

@test_factorial tests @factorial () -> void = run(
    assert_eq(actual: @factorial(n: 0), expected: 1),
    assert_eq(actual: @factorial(n: 1), expected: 1),
    assert_eq(actual: @factorial(n: 5), expected: 120)
)`,

    'list-ops': `// List operations
@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5],
    let doubled = numbers.map(transform: x -> x * 2),
    let evens = doubled.filter(predicate: x -> x % 2 == 0),
    let sum = evens.fold(initial: 0, op: (acc, x) -> acc + x),
    print(msg: "Sum of doubled evens: " + str(sum))
)`,

    'pattern-match': `// Pattern matching
type Shape = Circle(radius: float) | Rectangle(width: float, height: float)

@area (shape: Shape) -> float = match(
    shape,
    Circle(radius) -> 3.14159 * radius * radius,
    Rectangle(width, height) -> width * height
)

@main () -> void = run(
    let circle = Circle(radius: 5.0),
    let rect = Rectangle(width: 4.0, height: 3.0),
    print(msg: "Circle area: " + str(@area(shape: circle))),
    print(msg: "Rectangle area: " + str(@area(shape: rect)))
)

@test_area tests @area () -> void = run(
    assert_eq(
        actual: @area(shape: Rectangle(width: 2.0, height: 3.0)),
        expected: 6.0,
    )
)`
};

// Default code
const DEFAULT_CODE = `// Welcome to the Ori Playground!
// Write your code here and click Run (or press Ctrl+Enter)

@main () -> void = print(msg: "Hello from Ori!")`;

// State
let editor = null;
let wasmBytes = null;
let wasiReady = false;

// DOM Elements
const runBtn = document.getElementById('run-btn');
const shareBtn = document.getElementById('share-btn');
const examplesSelect = document.getElementById('examples');
const outputEl = document.getElementById('output');
const statusEl = document.getElementById('status');
const versionEl = document.getElementById('version');

// Initialize Monaco Editor
async function initMonaco() {
    return new Promise((resolve) => {
        require.config({
            paths: {
                vs: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs'
            }
        });

        require(['vs/editor/editor.main'], function () {
            // Register Ori language
            monaco.languages.register({ id: 'ori' });

            // Ori syntax highlighting
            monaco.languages.setMonarchTokensProvider('ori', {
                keywords: [
                    'if', 'then', 'else', 'let', 'mut', 'in', 'for', 'do', 'yield',
                    'match', 'type', 'trait', 'impl', 'pub', 'use', 'uses',
                    'true', 'false', 'self', 'Self', 'where', 'with', 'void',
                    'loop', 'break', 'continue', 'extension', 'extend'
                ],
                typeKeywords: [
                    'int', 'float', 'bool', 'str', 'char', 'byte', 'void',
                    'Option', 'Result', 'Some', 'None', 'Ok', 'Err',
                    'Never', 'Duration', 'Size'
                ],
                operators: [
                    '=', '>', '<', '!', '~', '?', ':', '==', '<=', '>=', '!=',
                    '&&', '||', '++', '--', '+', '-', '*', '/', '&', '|', '^',
                    '%', '<<', '>>', '->', '=>', '..', '..='
                ],
                symbols: /[=><!~?:&|+\-*\/\^%]+/,
                escapes: /\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,

                tokenizer: {
                    root: [
                        // Function definitions (@name)
                        [/@[a-zA-Z_]\w*/, 'entity.name.function'],

                        // Config variables ($name)
                        [/\$[a-zA-Z_]\w*/, 'variable.parameter'],

                        // Identifiers and keywords
                        [/[a-zA-Z_]\w*/, {
                            cases: {
                                '@keywords': 'keyword',
                                '@typeKeywords': 'type',
                                '@default': 'identifier'
                            }
                        }],

                        // Whitespace
                        { include: '@whitespace' },

                        // Delimiters
                        [/[{}()\[\]]/, '@brackets'],
                        [/[<>](?!@symbols)/, '@brackets'],
                        [/@symbols/, {
                            cases: {
                                '@operators': 'operator',
                                '@default': ''
                            }
                        }],

                        // Numbers
                        [/\d*\.\d+([eE][\-+]?\d+)?/, 'number.float'],
                        [/0[xX][0-9a-fA-F]+/, 'number.hex'],
                        [/\d+/, 'number'],

                        // Delimiter
                        [/[;,.]/, 'delimiter'],

                        // Strings
                        [/"([^"\\]|\\.)*$/, 'string.invalid'],
                        [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],

                        // Characters
                        [/'[^\\']'/, 'string'],
                        [/(')(@escapes)(')/, ['string', 'string.escape', 'string']],
                        [/'/, 'string.invalid']
                    ],

                    comment: [
                        [/[^\/*]+/, 'comment'],
                        [/\/\*/, 'comment', '@push'],
                        [/\*\//, 'comment', '@pop'],
                        [/[\/*]/, 'comment']
                    ],

                    string: [
                        [/[^\\"]+/, 'string'],
                        [/@escapes/, 'string.escape'],
                        [/\\./, 'string.escape.invalid'],
                        [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }]
                    ],

                    whitespace: [
                        [/[ \t\r\n]+/, 'white'],
                        [/\/\*/, 'comment', '@comment'],
                        [/\/\/.*$/, 'comment']
                    ]
                }
            });

            // Define VS Code Dark+ theme colors for Ori
            monaco.editor.defineTheme('ori-dark', {
                base: 'vs-dark',
                inherit: true,
                rules: [
                    { token: 'entity.name.function', foreground: 'DCDCAA' },
                    { token: 'variable.parameter', foreground: '9CDCFE' },
                    { token: 'keyword', foreground: '569CD6' },
                    { token: 'type', foreground: '4EC9B0' },
                    { token: 'string', foreground: 'CE9178' },
                    { token: 'number', foreground: 'B5CEA8' },
                    { token: 'comment', foreground: '6A9955' },
                    { token: 'operator', foreground: 'D4D4D4' }
                ],
                colors: {
                    'editor.background': '#1e1e1e',
                    'editor.foreground': '#d4d4d4',
                    'editorLineNumber.foreground': '#858585',
                    'editorCursor.foreground': '#aeafad',
                    'editor.selectionBackground': '#264f78',
                    'editor.lineHighlightBackground': '#2a2a2a'
                }
            });

            // Get initial code from URL hash or use default
            let initialCode = DEFAULT_CODE;
            if (window.location.hash) {
                try {
                    initialCode = decodeURIComponent(atob(window.location.hash.slice(1)));
                } catch (e) {
                    console.warn('Failed to decode URL hash:', e);
                }
            }

            // Create editor
            editor = monaco.editor.create(document.getElementById('editor'), {
                value: initialCode,
                language: 'ori',
                theme: 'ori-dark',
                fontSize: 14,
                fontFamily: "'JetBrains Mono', 'Consolas', 'Monaco', monospace",
                fontLigatures: true,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                automaticLayout: true,
                tabSize: 4,
                insertSpaces: true,
                renderWhitespace: 'selection',
                wordWrap: 'on',
                lineNumbers: 'on',
                glyphMargin: false,
                folding: true,
                lineDecorationsWidth: 10,
                lineNumbersMinChars: 3
            });

            // Keyboard shortcut: Ctrl+Enter to run
            editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
                runCode();
            });

            resolve(editor);
        });
    });
}

// Initialize WASM/WASI
async function initWasm() {
    try {
        // Load @wasmer/wasi from CDN
        const wasiModule = await import('https://unpkg.com/@aspect/run-wasi@0.5.0');
        await wasiModule.init();
        window.WasiModule = wasiModule;

        // Load the WASM binary
        const response = await fetch('./pkg/ori-playground.wasm');
        wasmBytes = await response.arrayBuffer();
        wasiReady = true;
        versionEl.textContent = 'Ori 0.1.0-alpha';
        return true;
    } catch (e) {
        console.error('Failed to load WASM/WASI:', e);
        versionEl.textContent = 'WASM not loaded';
        outputEl.textContent = `Note: WASM module not loaded.\n\nBuild with:\ncd playground/wasm && cargo build --target wasm32-wasi --release\nmkdir -p ../pkg && cp target/wasm32-wasi/release/ori-playground.wasm ../pkg/\n\nError: ${e.message}`;
        outputEl.className = 'output error';
        return false;
    }
}

// Run code
async function runCode() {
    if (!editor) return;

    const code = editor.getValue();

    if (!wasiReady || !wasmBytes) {
        outputEl.textContent = 'WASM module not loaded.\n\nBuild with:\ncd playground/wasm && cargo build --target wasm32-wasi --release';
        outputEl.className = 'output error';
        statusEl.textContent = 'Not Ready';
        statusEl.className = 'status error';
        return;
    }

    // Update UI
    runBtn.disabled = true;
    statusEl.textContent = 'Running...';
    statusEl.className = 'status running';
    outputEl.textContent = '';
    outputEl.className = 'output';

    try {
        // Run in next tick to allow UI to update
        await new Promise(resolve => setTimeout(resolve, 10));

        // Create WASI instance with stdin containing the source code
        const encoder = new TextEncoder();
        const stdinData = encoder.encode(code);

        const wasi = new window.WasiModule.WASI({
            args: ['ori-playground'],
            env: {},
            stdin: stdinData,
        });

        // Compile and instantiate
        const module = await WebAssembly.compile(wasmBytes);
        const instance = await wasi.instantiate(module, {});

        // Run
        const exitCode = wasi.start(instance);

        // Get output
        const stdout = wasi.getStdoutString();
        const stderr = wasi.getStderrString();

        if (exitCode === 0) {
            outputEl.textContent = stdout || '(no output)';
            outputEl.className = 'output success';
            statusEl.textContent = 'Success';
            statusEl.className = 'status success';
        } else {
            outputEl.textContent = stderr || stdout || 'Unknown error';
            outputEl.className = 'output error';
            statusEl.textContent = 'Error';
            statusEl.className = 'status error';
        }
    } catch (e) {
        outputEl.textContent = `Internal error: ${e.message}`;
        outputEl.className = 'output error';
        statusEl.textContent = 'Error';
        statusEl.className = 'status error';
    } finally {
        runBtn.disabled = false;
    }
}

// Share code (encode to URL)
function shareCode() {
    if (!editor) return;

    const code = editor.getValue();
    const encoded = btoa(encodeURIComponent(code));
    const url = `${window.location.origin}${window.location.pathname}#${encoded}`;

    navigator.clipboard.writeText(url).then(() => {
        const originalText = shareBtn.textContent;
        shareBtn.textContent = 'Copied!';
        setTimeout(() => {
            shareBtn.textContent = originalText;
        }, 2000);
    }).catch(() => {
        prompt('Copy this URL:', url);
    });
}

// Load example
function loadExample(name) {
    if (!editor || !name) return;

    const code = EXAMPLES[name];
    if (code) {
        editor.setValue(code);
        examplesSelect.value = '';
    }
}

// Event listeners
runBtn.addEventListener('click', runCode);
shareBtn.addEventListener('click', shareCode);
examplesSelect.addEventListener('change', (e) => loadExample(e.target.value));

// Initialize
async function init() {
    await Promise.all([
        initMonaco(),
        initWasm()
    ]);

    console.log('Ori Playground initialized');
}

init();
