import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { createRequire } from 'module';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

async function runTests() {
    // Load the WASM module
    const wasm = await import('./pkg/ori_playground_wasm.js');
    await wasm.default();
    
    const tests = [
        {
            name: "Basic print",
            code: `@main () -> void = print(msg: "hello")`,
            expectPrinted: "hello"
        },
        {
            name: "Simple struct",
            code: `
type Point = { x: int, y: int }

@main () -> void = run(
    let p = Point { x: 3, y: 4 },
    print(msg: str(p.x))
)`,
            expectPrinted: "3"
        },
        {
            name: "Impl block method",
            code: `
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> int = self.x + self.y
}

@main () -> void = run(
    let p = Point { x: 3, y: 4 },
    print(msg: str(p.sum()))
)`,
            expectPrinted: "7"
        }
    ];
    
    console.log("Running WASM tests...\n");
    
    let passed = 0;
    let failed = 0;
    
    for (const test of tests) {
        const resultJson = wasm.run_ori(test.code);
        const result = JSON.parse(resultJson);
        
        console.log(`Test: ${test.name}`);
        console.log(`  Success: ${result.success}`);
        console.log(`  Output: "${result.output}"`);
        console.log(`  Printed: "${result.printed}"`);
        if (result.error) {
            console.log(`  Error: ${result.error}`);
        }
        
        if (result.success && result.printed.trim() === test.expectPrinted) {
            console.log("  ✓ PASSED\n");
            passed++;
        } else {
            console.log(`  ✗ FAILED (expected printed: "${test.expectPrinted}")\n`);
            failed++;
        }
    }
    
    console.log(`\nResults: ${passed} passed, ${failed} failed`);
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(e => {
    console.error("Error:", e);
    process.exit(1);
});
