// Ori Playground Application

import initWasm, { run_ori, version } from './pkg/ori_playground_wasm.js';

// Example programs
const EXAMPLES = {
    hello: `// Hello World in Ori
@main () -> void = print(msg: "Hello, World!")`,

    fibonacci: `// Memoized - O(n) instead of O(2^n)
@fib (n: int) -> int = recurse(
    condition: n < 2,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
)

@main () -> void = run(
    print(msg: "fib(30) = " + str(fib(n: 30)))
)`,

    factorial: `// Factorial with recursion
@factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n: n - 1)

@main () -> void = run(
    print(msg: "5! = " + str(factorial(n: 5)))
)`,

    'list-ops': `// List operations
@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5],
    let doubled = numbers.map(transform: x -> x * 2),
    let evens = doubled.filter(predicate: x -> x % 2 == 0),
    let sum = evens.fold(initial: 0, op: (acc, x) -> acc + x),
    print(msg: "Sum of doubled evens: " + str(sum))
)`,

    'structs': `// Structs and methods
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> int = self.x + self.y
    @scale (self, factor: int) -> Point = Point { x: self.x * factor, y: self.y * factor }
}

@main () -> void = run(
    let p = Point { x: 3, y: 4 },
    print(msg: "Point sum: " + str(p.sum())),
    let scaled = p.scale(factor: 2),
    print(msg: "Scaled: (" + str(scaled.x) + ", " + str(scaled.y) + ")")
)`
};

// Default code
const DEFAULT_CODE = `// Welcome to the Ori Playground!
// Write your code here and click Run (or press Ctrl+Enter)

@main () -> void = print(msg: "Hello from Ori!")`;

// State
let editor = null;
let wasmReady = false;

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

// Initialize WASM module
async function initWasmModule() {
    try {
        await initWasm();
        wasmReady = true;
        versionEl.textContent = version();
        return true;
    } catch (e) {
        console.error('Failed to load WASM:', e);
        versionEl.textContent = 'WASM not loaded';
        outputEl.textContent = `Failed to load WASM module.\n\nBuild with:\ncd playground/wasm && wasm-pack build --target web --out-dir ../pkg\n\nError: ${e.message}`;
        outputEl.className = 'output error';
        return false;
    }
}

// Create the timing line element
function createTimingLine(elapsedStr) {
    const el = document.createElement('div');
    el.className = 'timing-line';
    const ran = document.createElement('span');
    ran.className = 'timing-duration';
    ran.textContent = `Ran in ${elapsedStr}`;
    const label = document.createElement('span');
    label.className = 'timing-label';
    label.textContent = 'interpreted in WASM';
    el.appendChild(ran);
    el.appendChild(document.createTextNode(' · '));
    el.appendChild(label);
    return el;
}

// Run code
async function runCode() {
    if (!editor) return;

    const code = editor.getValue();

    if (!wasmReady) {
        outputEl.textContent = 'WASM module not loaded.\n\nBuild with:\ncd playground/wasm && wasm-pack build --target web --out-dir ../pkg';
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

        // Call the WASM function and measure execution time
        const startTime = performance.now();
        const resultJson = run_ori(code);
        const elapsed = performance.now() - startTime;
        const result = JSON.parse(resultJson);

        const elapsedSec = elapsed / 1000;
        const elapsedStr = elapsedSec >= 0.01
            ? `${elapsedSec.toFixed(2)}s`
            : `${elapsed.toFixed(1)}ms`;

        if (result.success) {
            let output = '';
            if (result.printed) {
                output += result.printed;
            }
            if (result.output) {
                if (output) output += '\n';
                output += result.output;
            }
            outputEl.innerHTML = '';
            outputEl.appendChild(document.createTextNode(output || '(no output)'));
            outputEl.appendChild(createTimingLine(elapsedStr));
            outputEl.className = 'output success';
            statusEl.textContent = 'Success';
            statusEl.className = 'status success';
        } else {
            const errorType = result.error_type ? `[${result.error_type}] ` : '';
            outputEl.innerHTML = '';
            outputEl.appendChild(document.createTextNode(`${errorType}${result.error || 'Unknown error'}`));
            outputEl.appendChild(createTimingLine(elapsedStr));
            outputEl.className = 'output error';
            statusEl.textContent = result.error_type === 'parse' ? 'Parse Error' :
                                   result.error_type === 'type' ? 'Type Error' :
                                   'Runtime Error';
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
        initWasmModule()
    ]);

    console.log('Ori Playground initialized');
}

init();
