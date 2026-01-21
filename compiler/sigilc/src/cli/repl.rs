// REPL for Sigil
// Interactive read-eval-print loop

use sigilc::eval;
use std::io::{self, Write};

/// Start the REPL
pub fn repl() {
    let mut env = eval::Environment::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();

        match input {
            ":quit" | ":q" => break,
            ":help" | ":h" => {
                println!("Commands:");
                println!("  :quit, :q   Exit REPL");
                println!("  :help, :h   Show this help");
                println!("  :type <expr> Show type of expression");
            }
            _ if input.starts_with(":type ") => {
                let expr = &input[6..];
                match eval::type_of(expr, &env) {
                    Ok(t) => println!("{}", t),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            _ if input.is_empty() => continue,
            _ => match eval::eval_line(input, &mut env) {
                Ok(result) => {
                    if !result.is_empty() {
                        println!("{}", result);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            },
        }
    }
}
