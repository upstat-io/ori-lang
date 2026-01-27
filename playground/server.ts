const server = Bun.serve({
    port: 3000,
    async fetch(req) {
        const url = new URL(req.url);
        let path = url.pathname;

        // Default to index.html
        if (path === "/") {
            path = "/index.html";
        }

        // Serve static files from current directory
        const file = Bun.file(`.${path}`);

        if (await file.exists()) {
            // Set correct MIME type for WASM files
            const headers: Record<string, string> = {};
            if (path.endsWith(".wasm")) {
                headers["Content-Type"] = "application/wasm";
            }
            return new Response(file, { headers });
        }

        return new Response("Not Found", { status: 404 });
    },
});

console.log(`Ori Playground running at http://localhost:${server.port}`);
