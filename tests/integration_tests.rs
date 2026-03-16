use chip8_lang::analyzer::Analyzer;
use chip8_lang::codegen::CodeGen;
use chip8_lang::emitter;
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;
use std::path::Path;

fn compile_to_bytes(source: &str) -> Vec<u8> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut analyzer = Analyzer::new();
    analyzer.analyze(&program).unwrap();
    let mut codegen = CodeGen::new();
    codegen.generate(&program)
}

#[test]
fn test_full_pipeline() {
    let source = r#"
        fn main() -> () {
            clear();
        }
    "#;
    let bytes = compile_to_bytes(source);
    assert!(!bytes.is_empty());
    // 先頭は JP main
    assert_eq!(bytes[0] & 0xF0, 0x10);
}

#[test]
fn test_emit_to_file() {
    let source = r#"
        fn main() -> () {
            clear();
        }
    "#;
    let bytes = compile_to_bytes(source);

    let tmp_path = std::env::temp_dir().join("test_output.ch8");
    emitter::emit(&bytes, &tmp_path).unwrap();

    let read_back = std::fs::read(&tmp_path).unwrap();
    assert_eq!(bytes, read_back);

    // cleanup
    let _ = std::fs::remove_file(&tmp_path);
}

#[test]
fn test_rom_size_limit() {
    // 3584 バイトを超えるROMはエラー
    let big_bytes = vec![0u8; 4000];
    let tmp_path = std::env::temp_dir().join("test_big.ch8");
    let result = emitter::emit(&big_bytes, &tmp_path);
    assert!(result.is_err());
    let _ = std::fs::remove_file(&tmp_path);
}

#[test]
fn test_hello_example_compiles() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/hello.ch8l"))
            .unwrap();
    let bytes = compile_to_bytes(&source);
    assert!(!bytes.is_empty());
    assert_eq!(bytes[0] & 0xF0, 0x10);
}

#[test]
fn test_design_doc_program_e2e() {
    let source = r#"
        let BOARD_W: u8 = 10;
        let BOARD_H: u8 = 20;
        let block_sprite: sprite(1) = [0b11000000];

        fn draw_block(x: u8, y: u8) -> () {
            draw(block_sprite, x, y);
        }

        fn clamp(val: u8, max: u8) -> u8 {
            if val > max { max } else { val }
        }

        fn game_loop() -> () {
            loop {
                let key: u8 = wait_key();
                if key == 5 {
                    break;
                };
            };
        }

        fn main() -> () {
            clear();
            game_loop();
        }
    "#;
    let bytes = compile_to_bytes(source);
    assert!(!bytes.is_empty());

    // ファイルに書き出せることも確認
    let tmp_path = std::env::temp_dir().join("test_design.ch8");
    emitter::emit(&bytes, &tmp_path).unwrap();
    let read_back = std::fs::read(&tmp_path).unwrap();
    assert_eq!(bytes, read_back);
    let _ = std::fs::remove_file(&tmp_path);
}
