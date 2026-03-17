use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;
use chip8_lang::parser::ast::*;

fn parse(input: &str) -> Program {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse_program().unwrap()
}

#[test]
fn test_let_def() {
    let prog = parse("let X: u8 = 10;");
    assert_eq!(prog.top_levels.len(), 1);
    match &prog.top_levels[0] {
        TopLevel::LetDef { name, ty, .. } => {
            assert_eq!(name, "X");
            assert_eq!(*ty, Type::U8);
        }
        _ => panic!("expected LetDef"),
    }
}

#[test]
fn test_let_bool() {
    let prog = parse("let flag: bool = true;");
    match &prog.top_levels[0] {
        TopLevel::LetDef {
            name, ty, value, ..
        } => {
            assert_eq!(name, "flag");
            assert_eq!(*ty, Type::Bool);
            assert_eq!(value.kind, ExprKind::BoolLiteral(true));
        }
        _ => panic!("expected LetDef"),
    }
}

#[test]
fn test_sprite_type() {
    let prog = parse("let s: sprite(2) = [0b11000000, 0b00110000];");
    match &prog.top_levels[0] {
        TopLevel::LetDef { ty, .. } => {
            assert_eq!(*ty, Type::Sprite(2));
        }
        _ => panic!("expected LetDef"),
    }
}

#[test]
fn test_array_type() {
    let prog = parse("let arr: [u8; 3] = [1, 2, 3];");
    match &prog.top_levels[0] {
        TopLevel::LetDef { ty, value, .. } => {
            assert_eq!(*ty, Type::Array(Box::new(Type::U8), 3));
            if let ExprKind::ArrayLiteral(elems) = &value.kind {
                assert_eq!(elems.len(), 3);
            } else {
                panic!("expected ArrayLiteral");
            }
        }
        _ => panic!("expected LetDef"),
    }
}

#[test]
fn test_fn_def_simple() {
    let prog = parse("fn foo() -> () { }");
    match &prog.top_levels[0] {
        TopLevel::FnDef {
            name,
            params,
            return_type,
            ..
        } => {
            assert_eq!(name, "foo");
            assert!(params.is_empty());
            assert_eq!(*return_type, Type::Unit);
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_fn_def_with_params() {
    let prog = parse("fn add(a: u8, b: u8) -> u8 { a + b }");
    match &prog.top_levels[0] {
        TopLevel::FnDef {
            name,
            params,
            return_type,
            body,
            ..
        } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[0].ty, Type::U8);
            assert_eq!(params[1].name, "b");
            assert_eq!(params[1].ty, Type::U8);
            assert_eq!(*return_type, Type::U8);
            // body should be a Block with a tail expression
            if let ExprKind::Block { stmts, expr } = &body.kind {
                assert!(stmts.is_empty());
                assert!(expr.is_some());
            } else {
                panic!("expected Block");
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_binary_op_precedence() {
    // 1 + 2 * 3 should be 1 + (2 * 3)
    let prog = parse("fn f() -> u8 { 1 + 2 * 3 }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block {
                expr: Some(expr), ..
            } = &body.kind
            {
                if let ExprKind::BinaryOp { op, rhs, .. } = &expr.kind {
                    assert_eq!(*op, BinOp::Add);
                    if let ExprKind::BinaryOp { op: inner_op, .. } = &rhs.kind {
                        assert_eq!(*inner_op, BinOp::Mul);
                    } else {
                        panic!("expected Mul on rhs");
                    }
                } else {
                    panic!("expected BinaryOp");
                }
            } else {
                panic!("expected block with tail expr");
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_comparison_ops() {
    let prog = parse("fn f() -> bool { x > 5 }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block {
                expr: Some(expr), ..
            } = &body.kind
            {
                if let ExprKind::BinaryOp { op, .. } = &expr.kind {
                    assert_eq!(*op, BinOp::Gt);
                } else {
                    panic!("expected BinaryOp");
                }
            } else {
                panic!("expected block with tail expr");
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_if_expr() {
    let prog = parse("fn f(x: u8) -> u8 { if x > 5 { 1 } else { 0 } }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block {
                expr: Some(expr), ..
            } = &body.kind
            {
                if let ExprKind::If {
                    else_block: Some(_),
                    ..
                } = &expr.kind
                {
                    // OK
                } else {
                    panic!("expected If with else");
                }
            } else {
                panic!("expected block with tail expr");
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_loop_with_break() {
    let prog = parse(
        "fn f() -> () {
          loop {
            break;
          };
        }",
    );
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block { stmts, .. } = &body.kind {
                assert_eq!(stmts.len(), 1);
                if let StmtKind::Expr(ref expr) = stmts[0].kind {
                    assert!(matches!(expr.kind, ExprKind::Loop { .. }));
                } else {
                    panic!("expected Expr stmt");
                }
            } else {
                panic!("expected Block");
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_function_call() {
    let prog = parse("fn f() -> () { draw(s, x, y); }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block { stmts, .. } = &body.kind {
                assert_eq!(stmts.len(), 1);
                if let StmtKind::Expr(ref expr) = stmts[0].kind {
                    if let ExprKind::BuiltinCall { builtin, args } = &expr.kind {
                        assert_eq!(*builtin, BuiltinFunction::Draw);
                        assert_eq!(args.len(), 3);
                    } else {
                        panic!("expected BuiltinCall");
                    }
                }
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_let_stmt() {
    let prog = parse("fn f() -> () { let x: u8 = 42; }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block { stmts, .. } = &body.kind {
                assert_eq!(stmts.len(), 1);
                if let StmtKind::Let { name, ty, .. } = &stmts[0].kind {
                    assert_eq!(name, "x");
                    assert_eq!(*ty, Type::U8);
                } else {
                    panic!("expected Let");
                }
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_assign_stmt() {
    let prog = parse("fn f() -> () { x = 10; }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block { stmts, .. } = &body.kind {
                assert_eq!(stmts.len(), 1);
                if let StmtKind::Assign { name, .. } = &stmts[0].kind {
                    assert_eq!(name, "x");
                } else {
                    panic!("expected Assign");
                }
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_unary_neg() {
    let prog = parse("fn f() -> u8 { -x }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block {
                expr: Some(expr), ..
            } = &body.kind
            {
                if let ExprKind::UnaryOp { op, .. } = &expr.kind {
                    assert_eq!(*op, UnaryOp::Neg);
                } else {
                    panic!("expected UnaryOp");
                }
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_unary_not() {
    let prog = parse("fn f() -> bool { !flag }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block {
                expr: Some(expr), ..
            } = &body.kind
            {
                if let ExprKind::UnaryOp { op, .. } = &expr.kind {
                    assert_eq!(*op, UnaryOp::Not);
                } else {
                    panic!("expected UnaryOp");
                }
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_design_doc_full_parse() {
    let source = r#"
-- 定数定義
let BOARD_W: u8 = 10;
let BOARD_H: u8 = 20;

-- スプライト定義 (バイナリリテラル)
let block_sprite: sprite(1) = [0b11000000];

-- 関数定義
fn draw_block(x: u8, y: u8) -> () {
  draw(block_sprite, x, y);
}

-- 条件分岐 (if 式)
fn clamp(val: u8, max: u8) -> u8 {
  if val > max { max } else { val }
}

-- ループ (loop + break)
fn game_loop() -> () {
  loop {
    let key: u8 = wait_key();
    if key == 5 {
      break;
    };
  };
}

-- エントリーポイント
fn main() -> () {
  clear();
  game_loop();
}
"#;
    let prog = parse(source);
    // 5 top-level definitions
    assert_eq!(prog.top_levels.len(), 7);
}

#[test]
fn test_return_stmt() {
    let prog = parse("fn f() -> u8 { return 42; }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { body, .. } => {
            if let ExprKind::Block { stmts, .. } = &body.kind {
                assert_eq!(stmts.len(), 1);
                assert!(matches!(stmts[0].kind, StmtKind::Return(Some(_))));
            }
        }
        _ => panic!("expected FnDef"),
    }
}

#[test]
fn test_parse_error() {
    let mut lexer = Lexer::new("fn 42");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    assert!(parser.parse_program().is_err());
}
