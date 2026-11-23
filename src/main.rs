use std::io::{BufRead, IsTerminal, Write};

/// The person at the terminal: questions go to standard error (the list stays on standard output), answers come from standard input.
struct Stdin;

impl gloss::cli::Prompt for Stdin {
    fn is_terminal(&self) -> bool {
        std::io::stdin().is_terminal()
    }

    fn ask(&mut self, question: &str) -> Option<String> {
        // Never read input nobody is typing: a pipe or file would answer for the person.
        if !self.is_terminal() {
            return None;
        }
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(question.as_bytes());
        let _ = stderr.flush();
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => {
                let line = line.strip_suffix('\n').unwrap_or(&line);
                Some(line.strip_suffix('\r').unwrap_or(line).to_string())
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = gloss::cli::run(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        &mut Stdin,
    );
    std::process::exit(code);
}
