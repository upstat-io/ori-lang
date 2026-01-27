// Ori Language Server Protocol implementation
//
// Provides IDE features:
// - Syntax error diagnostics
// - Type error diagnostics
// - Hover information (types, documentation)
// - Go to definition
// - Code completion (basic)

mod server;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(server::OriLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
