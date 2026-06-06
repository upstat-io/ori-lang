#!/usr/bin/env node
// Tokenization harness for the Ori TextMate grammar.
//
// Usage:
//   node test/tokenize.js              run the built-in scope assertions
//   node test/tokenize.js <file.ori>   dump per-token scopes for a file
//
// Uses vscode-textmate + vscode-oniguruma (the engine VS Code itself runs).

'use strict';

const fs = require('fs');
const path = require('path');
const vsctm = require('vscode-textmate');
const oniguruma = require('vscode-oniguruma');

const GRAMMAR_PATH = path.join(__dirname, '..', 'syntaxes', 'ori.tmLanguage.json');

function createRegistry() {
    const wasmPath = path.join(path.dirname(require.resolve('vscode-oniguruma')), 'onig.wasm');
    const wasmBin = fs.readFileSync(wasmPath).buffer;
    const onigLib = oniguruma.loadWASM(wasmBin).then(() => ({
        createOnigScanner(patterns) { return new oniguruma.OnigScanner(patterns); },
        createOnigString(s) { return new oniguruma.OnigString(s); }
    }));
    return new vsctm.Registry({
        onigLib,
        loadGrammar(scopeName) {
            if (scopeName === 'source.ori') {
                return Promise.resolve(vsctm.parseRawGrammar(
                    fs.readFileSync(GRAMMAR_PATH).toString(), GRAMMAR_PATH));
            }
            return Promise.resolve(null);
        }
    });
}

function tokenizeLines(grammar, lines) {
    const result = [];
    let ruleStack = vsctm.INITIAL;
    for (const line of lines) {
        const r = grammar.tokenizeLine(line, ruleStack);
        result.push(r.tokens.map(t => ({
            text: line.substring(t.startIndex, t.endIndex),
            scopes: t.scopes
        })));
        ruleStack = r.ruleStack;
    }
    return result;
}

// Each case: lines to tokenize, then expectations against the LAST line.
// expect: [tokenText, scopeSubstring] — some token whose text contains
//   tokenText must carry a scope containing scopeSubstring.
// reject: [tokenText, scopeSubstring] — no token whose text contains
//   tokenText may carry a scope containing scopeSubstring.
const CASES = [
    { name: 'duration literal', lines: ['let $t = 100ms;'], expect: [['100ms', 'constant.numeric.duration.ori']] },
    { name: 'duration decimal', lines: ['let $t = 0.5s;'], expect: [['0.5s', 'constant.numeric.duration.ori']] },
    { name: 'duration ns', lines: ['let $t = 250ns;'], expect: [['250ns', 'constant.numeric.duration.ori']] },
    { name: 'size literal', lines: ['let $s = 1.5kb;'], expect: [['1.5kb', 'constant.numeric.size.ori']] },
    { name: 'size not duration', lines: ['let $s = 5mb;'], expect: [['5mb', 'constant.numeric.size.ori']], reject: [['5m', 'duration']] },
    { name: 'hex literal', lines: ['let $h = 0xDEAD_BEEF;'], expect: [['0xDEAD_BEEF', 'constant.numeric.hex.ori']] },
    { name: 'binary not size', lines: ['let $b = 0b1010;'], expect: [['0b1010', 'constant.numeric.binary.ori']], reject: [['0b', 'size']] },
    { name: 'underscored int', lines: ['let $n = 1_000_000;'], expect: [['1_000_000', 'constant.numeric.decimal.ori']] },
    { name: 'float exponent', lines: ['let $f = 2.5e-8;'], expect: [['2.5e-8', 'constant.numeric.float.ori']] },
    { name: 'function declaration', lines: ['@main () -> void = {'], expect: [['main', 'entity.name.function.ori'], ['->', 'keyword.operator.arrow.ori'], ['void', 'support.type.primitive.ori']] },
    { name: 'test declaration', lines: ['@t tests @add () -> void = assert(cond: true);'], expect: [['tests', 'keyword.declaration.ori'], ['add', 'entity.name.function.ori']] },
    { name: 'impl colon form', lines: ['impl Point: Eq {'], expect: [['impl', 'keyword.declaration.impl.ori'], ['Point', 'entity.name.type'], ['Eq', 'entity.other.inherited-class.ori']] },
    { name: 'trait decl', lines: ['trait Comparable: Eq {'], expect: [['Comparable', 'entity.name.type.declaration.ori']] },
    { name: 'string escapes', lines: ['let $s = "a\\n\\u{1F600}\\xFF";'], expect: [['\\n', 'constant.character.escape.ori'], ['\\u{1F600}', 'constant.character.escape.ori']] },
    { name: 'bad string escape', lines: ['let $s = "a\\q";'], expect: [['\\q', 'invalid.illegal.unrecognized-escape.ori']] },
    { name: 'template interpolation', lines: ['let $s = `value {x} end`;'], expect: [['x', 'meta.embedded.expression.ori']] },
    { name: 'template format spec', lines: ['let $s = `v {x:>10.2f}`;'], expect: [['>10.2f', 'constant.other.format-spec.ori']] },
    { name: 'template brace escape', lines: ['let $s = `lit {{brace}}`;'], expect: [['{{', 'constant.character.escape.brace.ori']] },
    { name: 'char literal', lines: ["let $c = 'a';"], expect: [['a', 'string.quoted.single.char.ori']] },
    { name: 'char hex escape', lines: ["let $c = '\\x41';"], expect: [['\\x41', 'constant.character.escape.ori']] },
    { name: 'byte literal', lines: ["let $b = b'\\xFF';"], expect: [['\\xFF', 'constant.character.escape.ori'], ["b", 'string.quoted.single.byte.ori']] },
    { name: 'attribute', lines: ['#derive(Eq, Clone)'], expect: [['derive', 'entity.other.attribute-name.ori'], ['Eq', 'entity.name.type']] },
    { name: 'file-level attribute', lines: ['#!target(os: "linux")'], expect: [['target', 'entity.other.attribute-name.ori']] },
    { name: 'length sugar not attribute', lines: ['let $l = xs[# - 1];'], expect: [['#', 'keyword.operator.length.ori']] },
    { name: 'label', lines: ['break:outer 42'], expect: [['outer', 'entity.name.label.ori']] },
    { name: 'loop label', lines: ['loop:outer {'], expect: [['outer', 'entity.name.label.ori'], ['loop', 'keyword.control.ori']] },
    { name: 'pattern keyword', lines: ['recurse('], expect: [['recurse', 'keyword.control.pattern.ori']] },
    { name: 'pattern word as identifier', lines: ['let $x = cache;'], reject: [['cache', 'keyword']] },
    { name: 'try block', lines: ['try {'], expect: [['try', 'keyword.control.pattern.ori']] },
    { name: 'pipe + named arg', lines: ['x |> f(over: xs)'], expect: [['|>', 'keyword.operator.pipe.ori'], ['over', 'variable.parameter.ori']] },
    { name: 'argument punning', lines: ['f(x:, y: 42)'], expect: [['x', 'variable.parameter.ori']] },
    { name: 'cast operators', lines: ['let $v = "42" as? int;'], expect: [['as', 'keyword.operator.word.cast.ori'], ['int', 'support.type.primitive.ori']] },
    { name: 'compound assignment', lines: ['x **= 2;'], expect: [['**=', 'keyword.operator.assignment.compound.ori']] },
    { name: 'inclusive range with by', lines: ['for i in 0..=10 by 2 do f(i:)'], expect: [['..=', 'keyword.operator.range.ori'], ['by', 'keyword.operator.word.range.ori']] },
    { name: 'spread', lines: ['let $all = [...a, ...b];'], expect: [['...', 'keyword.operator.spread.ori']] },
    { name: 'contracts', lines: ['pre(n > 0)'], expect: [['pre', 'keyword.other.contract.ori']] },
    { name: 'capabilities', lines: ['@get (url: str) -> str uses Http = fetch(url:);'], expect: [['uses', 'keyword.other.capability.ori'], ['Http', 'entity.name.type.ori']] },
    { name: 'doc comment member', lines: ['// * radius: the circle radius'], expect: [['radius', 'variable.other.member.doc.ori']] },
    { name: 'doc comment example', lines: ['// > add(1, 2) -> 3'], expect: [['add(1, 2) -> 3', 'markup.raw.code-example.ori']] },
    { name: 'extern block', lines: ['extern "c" from "m" {', '    @_sin (x: float) -> float as "sin"', '}'], expect: [['sin', 'string.quoted.double.ori']] },
    { name: 'ffi modifiers scoped to extern', lines: ['extern "c" from "db" {', '    @open (name: str, db: out CPtr) -> c_int', '}'], expect: [['out', 'storage.modifier.ffi.ori'], ['c_int', 'support.type.primitive.ffi.ori']] },
    { name: 'mut not keyword outside extern', lines: ['let $x = mut;'], reject: [['mut', 'storage.modifier']] },
    { name: 'no return keyword', lines: ['let $x = return;'], reject: [['return', 'keyword']] },
    { name: 'no async keyword', lines: ['let $x = async;'], reject: [['async', 'keyword']] },
    { name: 'future-reserved flagged', lines: ['let $x = static;'], expect: [['static', 'invalid.deprecated.reserved.ori']] },
    { name: 'compile-time for', lines: ['$for field in fields_of(T) yield field'], expect: [['for', 'keyword.control.compile-time.ori'], ['fields_of', 'support.function.builtin.compile-time.ori']] },
    { name: 'const binding', lines: ['let $timeout = 30s;'], expect: [['timeout', 'variable.other.constant.ori'], ['30s', 'constant.numeric.duration.ori']] },
    { name: 'builtin function', lines: ['print(msg: "hi")'], expect: [['print', 'support.function.builtin.ori']] },
    { name: 'channel constructor', lines: ['let (p, c) = channel<int>(buffer: 8);'], expect: [['channel', 'support.function.builtin.channel.ori']] },
    { name: 'variants', lines: ['let $r = Ok(42);'], expect: [['Ok', 'support.constant.variant.ori']] },
    { name: 'matmul spaced', lines: ['let $m = a @ b;'], expect: [['@', 'keyword.operator.arithmetic.matrix.ori']] },
    { name: 'wildcard', lines: ['_ -> 0,'], expect: [['_', 'variable.language.wildcard.ori']] },
    { name: 'def impl', lines: ['pub def impl Logger {'], expect: [['Logger', 'entity.other.inherited-class.ori']] }
];

function runAssertions(grammar) {
    let pass = 0;
    let fail = 0;
    for (const c of CASES) {
        const tokens = tokenizeLines(grammar, c.lines).flat();
        const problems = [];
        for (const [text, scopeFrag] of c.expect || []) {
            const hit = tokens.some(t => t.text.includes(text) && t.scopes.some(s => s.includes(scopeFrag)));
            if (!hit) problems.push(`MISSING ${JSON.stringify(text)} with scope ~${scopeFrag}`);
        }
        for (const [text, scopeFrag] of c.reject || []) {
            const hit = tokens.some(t => t.text.includes(text) && t.scopes.some(s => s.includes(scopeFrag)));
            if (hit) problems.push(`FORBIDDEN ${JSON.stringify(text)} carries scope ~${scopeFrag}`);
        }
        if (problems.length === 0) {
            pass++;
            console.log(`PASS ${c.name}`);
        } else {
            fail++;
            console.log(`FAIL ${c.name}`);
            for (const p of problems) console.log(`     ${p}`);
            for (const t of tokens) console.log(`     token ${JSON.stringify(t.text)} -> ${t.scopes.join(', ')}`);
        }
    }
    console.log(`\n${pass} passed, ${fail} failed`);
    return fail === 0;
}

function dumpFile(grammar, file) {
    const lines = fs.readFileSync(file, 'utf8').split('\n');
    const tokenized = tokenizeLines(grammar, lines);
    tokenized.forEach((tokens, i) => {
        console.log(`${String(i + 1).padStart(4)}: ${lines[i]}`);
        for (const t of tokens) {
            if (t.text.trim() === '') continue;
            console.log(`      ${JSON.stringify(t.text)} -> ${t.scopes.slice(1).join(', ')}`);
        }
    });
}

async function main() {
    const registry = createRegistry();
    const grammar = await registry.loadGrammar('source.ori');
    if (!grammar) {
        console.error('failed to load grammar');
        process.exit(1);
    }
    const file = process.argv[2];
    if (file) {
        dumpFile(grammar, file);
    } else {
        process.exit(runAssertions(grammar) ? 0 : 1);
    }
}

main().catch(err => { console.error(err); process.exit(1); });
