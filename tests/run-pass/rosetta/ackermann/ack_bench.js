function ack(m, n) {
    if (m === 0) return n + 1;
    if (n === 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}

// Warm up V8 JIT
for (let i = 0; i < 10; i++) ack(3, 5);

// Benchmark A(3,7)
let start = performance.now();
let result = ack(3, 7);
let elapsed = (performance.now() - start) / 1000;
console.log(`Node.js V8: A(3,7) = ${result}, ${elapsed.toFixed(3)}s (${Math.round(693964/elapsed)} calls/sec, ${(elapsed/693964*1e6).toFixed(2)} µs/call)`);

// Benchmark A(3,8)
start = performance.now();
result = ack(3, 8);
elapsed = (performance.now() - start) / 1000;
console.log(`Node.js V8: A(3,8) = ${result}, ${elapsed.toFixed(3)}s (${Math.round(2785999/elapsed)} calls/sec, ${(elapsed/2785999*1e6).toFixed(2)} µs/call)`);

// Benchmark A(4,1) — the real test
start = performance.now();
result = ack(4, 1);
elapsed = (performance.now() - start) / 1000;
console.log(`Node.js V8: A(4,1) = ${result}, ${elapsed.toFixed(3)}s`);
