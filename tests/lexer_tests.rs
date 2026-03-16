use chip8_lang::lexer::Lexer;
use chip8_lang::lexer::token::TokenKind;

fn kinds(input: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(input);
    lexer
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn test_empty_input() {
    assert_eq!(kinds(""), vec![TokenKind::Eof]);
}

#[test]
fn test_whitespace_only() {
    assert_eq!(kinds("   \n\t  "), vec![TokenKind::Eof]);
}

#[test]
fn test_decimal_number() {
    assert_eq!(kinds("42"), vec![TokenKind::IntLiteral(42), TokenKind::Eof]);
}

#[test]
fn test_hex_number() {
    assert_eq!(
        kinds("0xFF"),
        vec![TokenKind::IntLiteral(0xFF), TokenKind::Eof]
    );
}

#[test]
fn test_binary_number() {
    assert_eq!(
        kinds("0b11000000"),
        vec![TokenKind::IntLiteral(0b11000000), TokenKind::Eof]
    );
}

#[test]
fn test_keywords() {
    assert_eq!(
        kinds("let fn if else loop break return true false"),
        vec![
            TokenKind::Let,
            TokenKind::Fn,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::Loop,
            TokenKind::Break,
            TokenKind::Return,
            TokenKind::True,
            TokenKind::False,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_identifiers() {
    assert_eq!(
        kinds("foo bar_baz x1"),
        vec![
            TokenKind::Ident("foo".into()),
            TokenKind::Ident("bar_baz".into()),
            TokenKind::Ident("x1".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_keyword_vs_ident() {
    // "letter" starts with "let" but is an identifier
    assert_eq!(
        kinds("letter"),
        vec![TokenKind::Ident("letter".into()), TokenKind::Eof]
    );
}

#[test]
fn test_operators() {
    assert_eq!(
        kinds("+ - * / % == != < > <= >= && || ! ="),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::EqEq,
            TokenKind::NotEq,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::LtEq,
            TokenKind::GtEq,
            TokenKind::AndAnd,
            TokenKind::OrOr,
            TokenKind::Bang,
            TokenKind::Eq,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_punctuation() {
    assert_eq!(
        kinds("( ) { } [ ] , ; : ->"),
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::Colon,
            TokenKind::Arrow,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_comment_skip() {
    assert_eq!(
        kinds("let x -- this is a comment\nlet y"),
        vec![
            TokenKind::Let,
            TokenKind::Ident("x".into()),
            TokenKind::Let,
            TokenKind::Ident("y".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_let_binding() {
    assert_eq!(
        kinds("let BOARD_W: u8 = 10;"),
        vec![
            TokenKind::Let,
            TokenKind::Ident("BOARD_W".into()),
            TokenKind::Colon,
            TokenKind::Ident("u8".into()),
            TokenKind::Eq,
            TokenKind::IntLiteral(10),
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_function_definition() {
    assert_eq!(
        kinds("fn clamp(val: u8, max: u8) -> u8 { }"),
        vec![
            TokenKind::Fn,
            TokenKind::Ident("clamp".into()),
            TokenKind::LParen,
            TokenKind::Ident("val".into()),
            TokenKind::Colon,
            TokenKind::Ident("u8".into()),
            TokenKind::Comma,
            TokenKind::Ident("max".into()),
            TokenKind::Colon,
            TokenKind::Ident("u8".into()),
            TokenKind::RParen,
            TokenKind::Arrow,
            TokenKind::Ident("u8".into()),
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_sprite_definition() {
    assert_eq!(
        kinds("let block_sprite: sprite(1) = [0b11000000];"),
        vec![
            TokenKind::Let,
            TokenKind::Ident("block_sprite".into()),
            TokenKind::Colon,
            TokenKind::Ident("sprite".into()),
            TokenKind::LParen,
            TokenKind::IntLiteral(1),
            TokenKind::RParen,
            TokenKind::Eq,
            TokenKind::LBracket,
            TokenKind::IntLiteral(0b11000000),
            TokenKind::RBracket,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_span_tracking() {
    let mut lexer = Lexer::new("let x\nlet y");
    let tokens = lexer.tokenize().unwrap();
    // "let" at line 1, col 1
    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.column, 1);
    // "x" at line 1, col 5
    assert_eq!(tokens[1].span.line, 1);
    assert_eq!(tokens[1].span.column, 5);
    // "let" at line 2, col 1
    assert_eq!(tokens[2].span.line, 2);
    assert_eq!(tokens[2].span.column, 1);
    // "y" at line 2, col 5
    assert_eq!(tokens[3].span.line, 2);
    assert_eq!(tokens[3].span.column, 5);
}

#[test]
fn test_error_unexpected_char() {
    let mut lexer = Lexer::new("@");
    let err = lexer.tokenize().unwrap_err();
    assert!(err.message().contains("unexpected character"));
}

#[test]
fn test_error_invalid_hex() {
    let mut lexer = Lexer::new("0x");
    let err = lexer.tokenize().unwrap_err();
    assert!(err.message().contains("hex"));
}

#[test]
fn test_error_invalid_binary() {
    let mut lexer = Lexer::new("0b");
    let err = lexer.tokenize().unwrap_err();
    assert!(err.message().contains("binary"));
}

#[test]
fn test_design_doc_syntax() {
    // DESIGN.md の構文イメージ全体をトークナイズできることを確認
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
    let key = wait_key();
    if key == 5 {
      break;
    };
  }
}

-- エントリーポイント
fn main() -> () {
  clear();
  game_loop();
}
"#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    // 正常にトークナイズが完了し、最後が EOF であること
    assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    // 十分な数のトークンが生成されていること
    assert!(tokens.len() > 50);
}
