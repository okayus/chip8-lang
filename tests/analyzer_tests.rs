use chip8_lang::analyzer::{AnalyzeError, AnalyzeErrorKind, Analyzer};
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;
use chip8_lang::parser::ast::{BuiltinFunction, Type};

fn analyze(input: &str) -> Result<(), Vec<AnalyzeError>> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut analyzer = Analyzer::new();
    analyzer.analyze(&program)
}

fn analyze_ok(input: &str) {
    if let Err(errs) = analyze(input) {
        panic!("expected no errors, got: {:?}", errs);
    }
}

fn analyze_err_kind(input: &str, expected: AnalyzeErrorKind) {
    match analyze(input) {
        Ok(()) => panic!("expected error {:?}, got Ok", expected),
        Err(errs) => {
            assert!(
                errs.iter().any(|e| e.kind == expected),
                "expected {:?}, got: {:?}",
                expected,
                errs
            );
        }
    }
}

fn analyze_err_matches(input: &str, pred: impl Fn(&AnalyzeErrorKind) -> bool) {
    match analyze(input) {
        Ok(()) => panic!("expected error, got Ok"),
        Err(errs) => {
            assert!(
                errs.iter().any(|e| pred(&e.kind)),
                "no matching error in: {:?}",
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
    analyze_err_kind(
        "
        fn main() -> () {
            set_delay(x);
        }
    ",
        AnalyzeErrorKind::UndefinedVariable("x".into()),
    );
}

#[test]
fn test_undefined_function() {
    analyze_err_kind(
        "
        fn main() -> () {
            foo();
        }
    ",
        AnalyzeErrorKind::UndefinedFunction("foo".into()),
    );
}

#[test]
fn test_type_mismatch_in_let() {
    analyze_err_matches(
        "
        fn main() -> () {
            let x: bool = 42;
        }
    ",
        |k| {
            matches!(
                k,
                AnalyzeErrorKind::TypeMismatch {
                    context: "type mismatch in let",
                    ..
                }
            )
        },
    );
}

#[test]
fn test_return_type_mismatch() {
    analyze_err_matches(
        "
        fn f() -> u8 {
            true
        }
        fn main() -> () { }
    ",
        |k| {
            matches!(
                k,
                AnalyzeErrorKind::TypeMismatch {
                    context: "return type mismatch",
                    ..
                }
            )
        },
    );
}

#[test]
fn test_wrong_arg_count() {
    analyze_err_kind(
        "
        fn main() -> () {
            set_delay(1, 2);
        }
    ",
        AnalyzeErrorKind::BuiltinArgCountMismatch {
            builtin: BuiltinFunction::SetDelay,
            expected: 1,
            found: 2,
        },
    );
}

#[test]
fn test_break_outside_loop() {
    analyze_err_kind(
        "
        fn main() -> () {
            break;
        }
    ",
        AnalyzeErrorKind::BreakOutsideLoop,
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
    analyze_err_kind(
        "
        fn foo() -> () { }
    ",
        AnalyzeErrorKind::MissingMain,
    );
}

#[test]
fn test_if_else_type_mismatch() {
    analyze_err_matches(
        "
        fn f(x: u8) -> u8 {
            if x > 5 { 10 } else { true }
        }
        fn main() -> () { }
    ",
        |k| matches!(k, AnalyzeErrorKind::IfElseBranchMismatch { .. }),
    );
}

#[test]
fn test_if_condition_not_bool() {
    analyze_err_matches(
        "
        fn f(x: u8) -> u8 {
            if x { 10 } else { 0 }
        }
        fn main() -> () { }
    ",
        |k| matches!(k, AnalyzeErrorKind::IfConditionNotBool(_)),
    );
}

#[test]
fn test_logical_op_requires_bool() {
    analyze_err_matches(
        "
        fn f(x: u8, y: u8) -> bool {
            x && y
        }
        fn main() -> () { }
    ",
        |k| matches!(k, AnalyzeErrorKind::LogicalOpRequiresBool(_)),
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
    analyze_err_matches(
        "
        fn main() -> () {
            let x: u8 = 10;
            x = true;
        }
    ",
        |k| matches!(k, AnalyzeErrorKind::AssignmentTypeMismatch { .. }),
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

#[test]
fn test_too_many_locals() {
    // 11 パラメータ → 上限 10 を超えるのでエラー
    let params: Vec<String> = (0..11).map(|i| format!("p{i}: u8")).collect();
    let input = format!(
        "fn f({}) -> () {{ }} fn main() -> () {{ }}",
        params.join(", ")
    );
    analyze_err_matches(&input, |k| {
        matches!(k, AnalyzeErrorKind::TooManyLocals { .. })
    });
}

#[test]
fn test_max_locals_ok() {
    // 10 パラメータ → ちょうど上限なので OK
    let params: Vec<String> = (0..10).map(|i| format!("p{i}: u8")).collect();
    let input = format!(
        "fn f({}) -> () {{ }} fn main() -> () {{ }}",
        params.join(", ")
    );
    analyze_ok(&input);
}

#[test]
fn test_match_u8() {
    analyze_ok(
        "fn main() -> u8 {
            let x: u8 = 1;
            match x {
                0 => 10,
                1 => 20,
                2 => 30,
            }
        }",
    );
}

#[test]
fn test_match_scrutinee_not_u8_or_enum() {
    analyze_err_kind(
        "fn main() -> u8 {
            let x: bool = true;
            match x {
                0 => 10,
                1 => 20,
            }
        }",
        AnalyzeErrorKind::MatchScrutineeType(Type::Bool),
    );
}

#[test]
fn test_match_arm_type_mismatch() {
    analyze_err_kind(
        "fn main() -> u8 {
            let x: u8 = 1;
            match x {
                0 => 10,
                1 => true,
            }
        }",
        AnalyzeErrorKind::MatchArmTypeMismatch {
            first: Type::U8,
            found: Type::Bool,
        },
    );
}

#[test]
fn test_match_no_arms() {
    analyze_err_kind(
        "fn main() -> () {
            let x: u8 = 1;
            match x {};
        }",
        AnalyzeErrorKind::MatchNoArms,
    );
}

#[test]
fn test_enum_definition_and_use() {
    analyze_ok(
        "enum Dir { Up, Down, Left, Right }
         fn main() -> Dir {
            Dir::Up
         }",
    );
}

#[test]
fn test_enum_undefined_variant() {
    analyze_err_kind(
        "enum Dir { Up, Down }
         fn main() -> Dir {
            Dir::Left
         }",
        AnalyzeErrorKind::UndefinedEnumVariant {
            enum_name: "Dir".to_string(),
            variant: "Left".to_string(),
        },
    );
}

#[test]
fn test_enum_undefined_enum() {
    analyze_err_kind(
        "fn main() -> () {
            let x: u8 = 0;
            Foo::Bar;
         }",
        AnalyzeErrorKind::UndefinedEnum("Foo".to_string()),
    );
}

#[test]
fn test_match_enum_exhaustive() {
    analyze_ok(
        "enum Dir { Up, Down }
         fn main() -> u8 {
            let d: Dir = Dir::Up;
            match d {
                Dir::Up => 1,
                Dir::Down => 2,
            }
         }",
    );
}

#[test]
fn test_match_enum_non_exhaustive() {
    analyze_err_kind(
        "enum Dir { Up, Down, Left }
         fn main() -> u8 {
            let d: Dir = Dir::Up;
            match d {
                Dir::Up => 1,
                Dir::Down => 2,
            }
         }",
        AnalyzeErrorKind::NonExhaustiveMatch {
            enum_name: "Dir".to_string(),
            missing: vec!["Left".to_string()],
        },
    );
}

#[test]
fn test_unknown_type() {
    analyze_err_kind(
        "fn main() -> () {
            let x: Foo = 0;
         }",
        AnalyzeErrorKind::UnknownType("Foo".to_string()),
    );
}

#[test]
fn test_random_enum_ok() {
    analyze_ok(
        "enum Piece { I, O, T, S, Z, L, J }
         fn main() -> Piece { random_enum(Piece) }",
    );
}

#[test]
fn test_random_enum_returns_enum_type() {
    // random_enum(Piece) の戻り値を Piece 型変数に代入できること
    analyze_ok(
        "enum Dir { Up, Down, Left, Right }
         fn main() -> Dir {
            let d: Dir = random_enum(Dir);
            d
         }",
    );
}

#[test]
fn test_random_enum_not_enum_name() {
    analyze_err_kind(
        "fn main() -> u8 { random_enum(Foo) }",
        AnalyzeErrorKind::RandomEnumArgNotEnum("Foo".to_string()),
    );
}

#[test]
fn test_random_enum_wrong_arg_count() {
    analyze_err_kind(
        "enum A { X }
         fn main() -> A { random_enum(A, A) }",
        AnalyzeErrorKind::BuiltinArgCountMismatch {
            builtin: BuiltinFunction::RandomEnum,
            expected: 1,
            found: 2,
        },
    );
}

#[test]
fn test_struct_basic() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn main() -> Pos { Pos { x: 1, y: 2 } }",
    );
}

#[test]
fn test_struct_field_access() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn main() -> u8 {
            let p: Pos = Pos { x: 10, y: 20 };
            p.x
         }",
    );
}

#[test]
fn test_struct_undefined_field() {
    analyze_err_kind(
        "struct Pos { x: u8, y: u8 }
         fn main() -> Pos { Pos { x: 1, z: 2 } }",
        AnalyzeErrorKind::UndefinedField {
            struct_name: "Pos".to_string(),
            field: "z".to_string(),
        },
    );
}

#[test]
fn test_struct_missing_fields() {
    analyze_err_matches(
        "struct Pos { x: u8, y: u8 }
         fn main() -> Pos { Pos { x: 1 } }",
        |k| matches!(k, AnalyzeErrorKind::MissingFields { .. }),
    );
}

#[test]
fn test_struct_field_access_on_non_struct() {
    analyze_err_matches(
        "fn main() -> u8 {
            let x: u8 = 1;
            x.field
         }",
        |k| matches!(k, AnalyzeErrorKind::FieldAccessOnNonStruct(_)),
    );
}

#[test]
fn test_struct_update_syntax() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn main() -> Pos {
            let p: Pos = Pos { x: 1, y: 2 };
            Pos { ..p, x: 10 }
         }",
    );
}

#[test]
fn test_struct_as_param_and_return() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn get_x(p: Pos) -> u8 { p.x }
         fn main() -> u8 {
            let p: Pos = Pos { x: 5, y: 10 };
            get_x(p)
         }",
    );
}

#[test]
fn test_enum_equality() {
    analyze_ok(
        "enum Dir { Up, Down }
         fn main() -> bool {
            let d: Dir = Dir::Up;
            d == Dir::Up
         }",
    );
}

#[test]
fn test_struct_equality() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn main() -> bool {
            let a: Pos = Pos { x: 1, y: 2 };
            let b: Pos = Pos { x: 1, y: 2 };
            a == b
         }",
    );
}

#[test]
fn test_struct_inequality() {
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn main() -> bool {
            let a: Pos = Pos { x: 1, y: 2 };
            let b: Pos = Pos { x: 3, y: 2 };
            a != b
         }",
    );
}

#[test]
fn test_struct_locals_dont_count_toward_register_limit() {
    // struct 型ローカルはメモリに配置されるため、レジスタ上限にカウントされない
    // 8 スカラー + 2 struct = スカラーのみ 8 なので OK (上限 10)
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn f(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8, h: u8) -> u8 {
            let p1: Pos = Pos { x: 1, y: 2 };
            let p2: Pos = Pos { x: 3, y: 4 };
            a
         }
         fn main() -> () { }",
    );
}

#[test]
fn test_many_struct_params_no_overflow() {
    // struct パラメータが多くてもレジスタ上限に引っかからない
    analyze_ok(
        "struct Pos { x: u8, y: u8 }
         fn f(a: Pos, b: Pos, c: Pos, d: Pos, e: Pos) -> u8 {
            a.x
         }
         fn main() -> () { }",
    );
}

#[test]
fn test_mutable_global_ok() {
    analyze_ok(
        "let mut score: u8 = 0;
         fn main() -> () {
            score = 10;
         }",
    );
}

#[test]
fn test_immutable_global_assign_error() {
    analyze_err_matches(
        "let score: u8 = 0;
         fn main() -> () {
            score = 10;
         }",
        |k| matches!(k, AnalyzeErrorKind::ImmutableAssignment(_)),
    );
}

#[test]
fn test_array_index_assign_ok() {
    analyze_ok(
        "let mut board: [u8; 4] = [0, 0, 0, 0];
         fn main() -> () {
            board[2] = 42;
         }",
    );
}

#[test]
fn test_immutable_array_assign_error() {
    analyze_err_matches(
        "let board: [u8; 4] = [0, 0, 0, 0];
         fn main() -> () {
            board[2] = 42;
         }",
        |k| matches!(k, AnalyzeErrorKind::ImmutableAssignment(_)),
    );
}
