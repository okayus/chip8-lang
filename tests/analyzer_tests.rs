use chip8_lang::analyzer::Analyzer;
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;

fn analyze(input: &str) -> Result<(), Vec<String>> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut analyzer = Analyzer::new();
    analyzer
        .analyze(&program)
        .map_err(|errs| errs.into_iter().map(|e| e.message).collect())
}

fn analyze_ok(input: &str) {
    if let Err(errs) = analyze(input) {
        panic!("expected no errors, got: {:?}", errs);
    }
}

fn analyze_err(input: &str, expected_msg: &str) {
    match analyze(input) {
        Ok(()) => panic!("expected error containing '{expected_msg}', got Ok"),
        Err(errs) => {
            assert!(
                errs.iter().any(|e| e.contains(expected_msg)),
                "expected error containing '{expected_msg}', got: {:?}",
                errs
            );
        }
    }
}

#[test]
fn test_simple_program() {
    analyze_ok(
        "
        fn main() -> () {
            clear();
        }
    ",
    );
}

#[test]
fn test_let_and_use() {
    analyze_ok(
        "
        fn main() -> () {
            let x: u8 = 10;
            set_delay(x);
        }
    ",
    );
}

#[test]
fn test_if_else_types_match() {
    analyze_ok(
        "
        fn f(x: u8) -> u8 {
            if x > 5 { 10 } else { 0 }
        }
        fn main() -> () { }
    ",
    );
}

#[test]
fn test_undefined_variable() {
    analyze_err(
        "
        fn main() -> () {
            set_delay(x);
        }
    ",
        "undefined variable: 'x'",
    );
}

#[test]
fn test_undefined_function() {
    analyze_err(
        "
        fn main() -> () {
            foo();
        }
    ",
        "undefined function: 'foo'",
    );
}

#[test]
fn test_type_mismatch_in_let() {
    analyze_err(
        "
        fn main() -> () {
            let x: bool = 42;
        }
    ",
        "type mismatch in let",
    );
}

#[test]
fn test_return_type_mismatch() {
    analyze_err(
        "
        fn f() -> u8 {
            true
        }
        fn main() -> () { }
    ",
        "return type mismatch",
    );
}

#[test]
fn test_wrong_arg_count() {
    analyze_err(
        "
        fn main() -> () {
            set_delay(1, 2);
        }
    ",
        "expects 1 args, got 2",
    );
}

#[test]
fn test_break_outside_loop() {
    analyze_err(
        "
        fn main() -> () {
            break;
        }
    ",
        "break outside of loop",
    );
}

#[test]
fn test_break_inside_loop() {
    analyze_ok(
        "
        fn main() -> () {
            loop {
                break;
            };
        }
    ",
    );
}

#[test]
fn test_missing_main() {
    analyze_err(
        "
        fn foo() -> () { }
    ",
        "missing 'main' function",
    );
}

#[test]
fn test_if_else_type_mismatch() {
    analyze_err(
        "
        fn f(x: u8) -> u8 {
            if x > 5 { 10 } else { true }
        }
        fn main() -> () { }
    ",
        "if/else type mismatch",
    );
}

#[test]
fn test_if_condition_not_bool() {
    analyze_err(
        "
        fn f(x: u8) -> u8 {
            if x { 10 } else { 0 }
        }
        fn main() -> () { }
    ",
        "if condition must be bool",
    );
}

#[test]
fn test_logical_op_requires_bool() {
    analyze_err(
        "
        fn f(x: u8, y: u8) -> bool {
            x && y
        }
        fn main() -> () { }
    ",
        "logical op requires bool",
    );
}

#[test]
fn test_function_call_between_user_fns() {
    analyze_ok(
        "
        fn helper() -> u8 { 42 }
        fn main() -> () {
            let x: u8 = helper();
        }
    ",
    );
}

#[test]
fn test_sprite_and_draw() {
    analyze_ok(
        "
        let s: sprite(1) = [0b11000000];
        fn main() -> () {
            let x: u8 = 10;
            let y: u8 = 20;
            draw(s, x, y);
        }
    ",
    );
}

#[test]
fn test_assign_type_mismatch() {
    analyze_err(
        "
        fn main() -> () {
            let x: u8 = 10;
            x = true;
        }
    ",
        "assignment type mismatch",
    );
}

#[test]
fn test_design_doc_program() {
    analyze_ok(
        r#"
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
    "#,
    );
}
