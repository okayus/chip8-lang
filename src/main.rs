use std::path::Path;
use std::process;

use chip8_lang::analyzer::Analyzer;
use chip8_lang::codegen::CodeGen;
use chip8_lang::emitter;
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: chip8-lang <source.ch8l>");
        process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let source = match std::fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", input_path.display(), e);
            process::exit(1);
        }
    };

    // Lexer
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {e}");
            process::exit(1);
        }
    };

    // Parser
    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    };

    // Analyzer
    let mut analyzer = Analyzer::new();
    if let Err(errors) = analyzer.analyze(&program) {
        for e in &errors {
            eprintln!("Analyze error: {e}");
        }
        process::exit(1);
    }

    // CodeGen
    let mut codegen = CodeGen::new();
    let bytes = codegen.generate(&program);

    // Emitter
    let output_path = input_path.with_extension("ch8");
    match emitter::emit(&bytes, &output_path) {
        Ok(()) => {
            println!(
                "Compiled {} -> {} ({} bytes)",
                input_path.display(),
                output_path.display(),
                bytes.len()
            );
        }
        Err(e) => {
            eprintln!("Emit error: {e}");
            process::exit(1);
        }
    }
}
