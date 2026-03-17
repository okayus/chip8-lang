# CHIP-8 言語実装：ビギナー向け解説ガイド

## 概要

このプロジェクトは、CHIP-8 ROM にコンパイルできる**関数型言語の処理系**を Rust で実装したものです。
コンパイラの全フェーズ（字句解析→構文解析→意味解析→コード生成→出力）を一通り体験できるよう設計されています。

---

## 1. 全体コンパイラパイプラインと主要モジュール

### パイプライン概要

```
ソースコード (.ch8l)
    ↓ [Lexer]
トークン列
    ↓ [Parser]
抽象構文木 (AST)
    ↓ [Analyzer]
型チェック・スコープ解決済み AST
    ↓ [CodeGen]
CHIP-8 バイトコード
    ↓ [Emitter]
.ch8 バイナリ ROM ファイル
```

**ソースコード参照**：
- **エントリーポイント**: `/home/okayu/dev/challenge/chip8/chip8-lang/src/main.rs` (76行)
  - 各フェーズの実行順序を示すメイン関数がここに
  - エラーハンドリングの基本パターンも参考になる

- **ライブラリインターフェース**: `/home/okayu/dev/challenge/chip8/chip8-lang/src/lib.rs` (19行)
  - 6つのモジュールを pub で公開
  - テスト・統合時のインポート基点になる

### 6つの主要モジュール

| モジュール | ファイル | 行数 | 役割 |
|-----------|---------|------|------|
| **lexer** | `src/lexer/mod.rs` | 397 | ソース → トークン列 |
| **parser** | `src/parser/mod.rs` + `ast.rs` | 871+272 | トークン → AST |
| **analyzer** | `src/analyzer/mod.rs` | 1056 | 型チェック・スコープ解決 |
| **codegen** | `src/codegen/mod.rs` | 1623 | AST → CHIP-8 バイトコード |
| **emitter** | `src/emitter/mod.rs` | 39 | バイトコード → .ch8 ファイル |
| **chip8** | `src/chip8.rs` | 360 | CHIP-8 ドメイン型 |

**全体コード量**: 4794行 (依存なしの純粋 Rust)

---

## 2. コア ドメイン型：言語とハードウェアの表現

### 2.1 CHIP-8 ハードウェアドメイン型 (`src/chip8.rs`, 360行)

言語処理系の根底にあるのは、**型を使ったドメインモデリング**です。生の `u8` や `String` ではなく、意味を持つ newtype と enum を使用します。

#### レジスタ (`Register`)
```rust
// ユーザー割り当て可能レジスタ V0-VE (14個)
pub struct UserRegister(u8);  // 0-14 のみ許可

// VF はフラグレジスタで専用
pub enum Register {
    User(UserRegister),
    Flag,
}
```
**テスト**: `src/chip8.rs` 行225-360
- `test_cls`, `test_jp`, `test_alu_ops` など各命令エンコーディングを検証

#### メモリアドレス (`Addr`)
```rust
pub struct Addr(u16);  // 12bit (0x000-0xFFF)
impl Addr {
    pub const PROGRAM_START: Self = Self(0x200);  // ユーザープログラム開始位置
}
```

#### スプライトの高さ (`SpriteHeight`)
```rust
pub struct SpriteHeight(u8);  // 1-15 のみ有効
```

#### CHIP-8 命令セット (`Opcode` enum)
```rust
pub enum Opcode {
    Cls,                           // 00E0 - 画面クリア
    Call(Addr),                    // 2NNN - サブルーチン呼び出し
    LdImm(Register, u8),           // 6XKK - レジスタに即値ロード
    Drw(Register, Register, SpriteHeight),  // DXYN - スプライト描画
    // ... 約35個のバリアント
}

impl Opcode {
    pub fn encode(self) -> [u8; 2] { /* ビッグエンディアン 2 バイトに変換 */ }
}
```

**デザイン思想**:
- 各 Opcode バリアントが 1 つの命令に対応
- オペランドの型が厳密に定義されている
- コンパイラは不正なオペランド（例：V16）を作れない
- 結果として unsafe な bit 操作が不要

---

### 2.2 言語の AST 型 (`src/parser/ast.rs`, 272行)

#### 基本型 (`Type` enum)
```rust
pub enum Type {
    U8,
    Bool,
    Unit,
    Array(Box<Type>, usize),
    Sprite(usize),
    UserType(String),
}
```

#### 式と文
```rust
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,  // エラー報告用の位置情報
}

pub enum ExprKind {
    IntLiteral(u64),
    Ident(String),
    BinaryOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    If { cond: Box<Expr>, then_block: Box<Expr>, else_block: Option<Box<Expr>> },
    Loop { body: Box<Expr> },
    Block { stmts: Vec<Stmt>, expr: Option<Box<Expr>> },
    BuiltinCall { builtin: BuiltinFunction, args: Vec<Expr> },
    Match { scrutinee: Box<Expr>, arms: Vec<MatchArm> },
    // ... など
}

pub enum StmtKind {
    Let { name: String, ty: Type, value: Expr },
    Assign { name: String, value: Expr },
    Return(Option<Expr>),
    Break,
    // ...
}
```

#### 組み込み関数 (`BuiltinFunction` enum)
```rust
pub enum BuiltinFunction {
    Clear,              // clear() - 画面クリア
    Draw,               // draw(sprite, x, y) -> bool
    WaitKey,            // wait_key() -> u8
    IsKeyPressed,       // is_key_pressed(k) -> bool
    Random,             // random(mask) -> u8
    // ... など
}

impl BuiltinFunction {
    pub fn from_name(name: &str) -> Option<Self> { /* 文字列→enum 変換 */ }
    pub fn signature(self) -> (Vec<Type>, Type) { /* 型シグネチャ */ }
}
```

**デザイン思想**:
- 「文字列分岐」を避ける：関数名は enum バリアントで管理
- stringly-typed コードを排除し、型安全に
- analyzer と codegen で確実に対応する実装を強制

---

## 3. 各フェーズの構造と実装パターン

### 3.1 字句解析 (Lexer) - `src/lexer/mod.rs` (397行)

**目的**: ソース文字列 → トークン列

#### トークン型 (`src/lexer/token.rs`, 81行)
```rust
pub enum TokenKind {
    // キーワード
    Let, Fn, If, Else, Loop, Break, Return, True, False, Match, Enum, Struct, Mut,
    // リテラル
    IntLiteral(u64),
    // 識別子
    Ident(String),
    // 演算子・区切り
    Plus, Minus, Star, Slash, Eq, EqEq, NotEq, Arrow, FatArrow, // ...
    Eof,
}

pub struct Span {
    pub line: usize,
    pub column: usize,
}
```

#### Lexer 実装パターン
```rust
pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self { /* 初期化 */ }
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof { break; }
        }
        Ok(tokens)
    }
    // 内部: peek(), advance(), scan_number() など
}
```

#### エラー型
```rust
pub enum LexErrorKind {
    InvalidNumber { literal: String, base: &'static str },
    ExpectedDigitsAfterPrefix { prefix: &'static str },
    UnexpectedCharacter(char),
}
```

**ビギナーのコツ**:
- `pos` でカーソル位置を管理、毎回 `line/column` を更新
- `next_token()` で 1 トークンを返し、EOF に達するまで繰り返し
- キーワード vs 識別子の判定は予約語リストで

---

### 3.2 構文解析 (Parser) - `src/parser/mod.rs` (871行)

**目的**: トークン列 → 抽象構文木 (AST)

#### Parser の基本構造
```rust
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0 } }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut top_levels = Vec::new();
        while !self.is_at_end() {
            top_levels.push(self.parse_top_level()?);
        }
        Ok(Program { top_levels })
    }

    // 内部メソッド
    fn parse_top_level(&mut self) -> Result<TopLevel, ParseError> { ... }
    fn parse_fn_def(&mut self) -> Result<TopLevel, ParseError> { ... }
    fn parse_expr(&mut self) -> Result<Expr, ParseError> { ... }
    fn parse_expr_primary(&mut self) -> Result<Expr, ParseError> { ... }
    // ... など
}
```

#### 再帰下降パースの例
```rust
fn parse_expr(&mut self) -> Result<Expr, ParseError> {
    // Pratt parsing: 優先順位に従い左結合的に解析
    self.parse_or()  // 最も優先順位が低い演算子から開始
}

fn parse_or(&mut self) -> Result<Expr, ParseError> {
    let mut expr = self.parse_and()?;  // 次の優先順位
    while self.peek() == &TokenKind::OrOr {
        self.advance();
        let rhs = self.parse_and()?;
        expr = Expr {
            kind: ExprKind::BinaryOp { op: BinOp::Or, lhs: Box::new(expr), rhs: Box::new(rhs) },
            span: expr.span,
        };
    }
    Ok(expr)
}
```

**エラー型**:
```rust
pub enum ParseErrorKind {
    UnexpectedToken { expected: String, found: TokenKind },
    UnknownType(String),
    InvalidAssignmentTarget,
}
```

**ビギナーのコツ**:
- トークン位置は `pos` で管理し、`peek()` で先読み
- 演算子は優先度の低い順に `parse_or()` → `parse_and()` → ... → `parse_primary()` の層構造
- 各ルールは「1 つの非終端記号」に対応させる

---

### 3.3 意味解析 (Analyzer) - `src/analyzer/mod.rs` (1056行)

**目的**: 型チェック、変数・関数の定義確認、CHIP-8 制約の検証

#### Analyzer エラー型 (一部)
```rust
pub enum AnalyzeErrorKind {
    MissingMain,
    UndefinedVariable(String),
    UndefinedFunction(String),
    TypeMismatch {
        context: &'static str,
        expected: Type,
        found: Type,
    },
    BinaryOpTypeMismatch { lhs: Type, rhs: Type },
    ArgumentCountMismatch { function: String, expected: usize, found: usize },
    TooManyLocals { count: usize, max: usize },  // V0-VE は15個のみ
    BreakOutsideLoop,
    NonExhaustiveMatch { enum_name: String, missing: Vec<String> },
    // ... 約 25 種類
}

pub struct AnalyzeError {
    pub kind: AnalyzeErrorKind,
    pub span: Span,
}
```

#### Analyzer 実装パターン
```rust
pub struct Analyzer {
    // グローバルスコープ
    global_functions: HashMap<String, (Vec<Type>, Type)>,
    global_vars: HashMap<String, Type>,
    // ローカルスコープ (関数内で初期化)
    local_vars: HashMap<String, Type>,
    enum_defs: HashMap<String, Vec<String>>,
    struct_defs: HashMap<String, Vec<StructField>>,
}

impl Analyzer {
    pub fn new() -> Self { /* ... */ }

    pub fn analyze(&mut self, program: &Program) -> Result<(), Vec<AnalyzeError>> {
        // Pass 1: グローバル定義を収集
        self.collect_definitions(program)?;
        // Pass 2: 関数本体を検査
        self.check_functions(program)?;
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, Vec<AnalyzeError>> {
        match &expr.kind {
            ExprKind::IntLiteral(_) => Ok(Type::U8),
            ExprKind::BoolLiteral(_) => Ok(Type::Bool),
            ExprKind::Ident(name) => self.lookup_variable(name),
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.check_expr(lhs)?;
                let rhs_ty = self.check_expr(rhs)?;
                self.check_binop(*op, lhs_ty, rhs_ty)
            }
            ExprKind::BuiltinCall { builtin, args } => {
                // 組み込み関数のシグネチャと照合
                let (param_types, return_type) = builtin.signature();
                if args.len() != param_types.len() {
                    return Err(vec![AnalyzeError {
                        kind: AnalyzeErrorKind::BuiltinArgCountMismatch {
                            builtin: *builtin,
                            expected: param_types.len(),
                            found: args.len(),
                        },
                        span: expr.span,
                    }]);
                }
                // 各引数の型を確認
                for (arg, param_type) in args.iter().zip(param_types.iter()) {
                    let arg_type = self.check_expr(arg)?;
                    if arg_type != *param_type {
                        return Err(vec![AnalyzeError { /* ... */ }]);
                    }
                }
                Ok(return_type)
            }
            // ... 他の ExprKind パターン
        }
    }
}
```

**ビギナーのコツ**:
- 2 Pass 設計：先に全定義を収集してから個別チェック
- 各式は「その式の型」を返す関数 `check_expr()` として実装
- エラーは全て収集してから報告（ユーザーが複数エラーを一度に見られる）

---

### 3.4 コード生成 (CodeGen) - `src/codegen/mod.rs` (1623行)

**目的**: AST → CHIP-8 バイトコード

#### CodeGen の核となるデータ構造
```rust
pub struct CodeGen {
    bytes: Vec<u8>,                          // 生成バイトコード
    data: Vec<u8>,                           // スプライトなどのデータセクション
    fn_addrs: HashMap<String, Addr>,         // 関数名→アドレス
    data_offsets: HashMap<String, u16>,      // グローバル変数→メモリオフセット
    enum_variant_values: HashMap<(String, String), u8>,  // enum variant の値
    struct_defs: HashMap<String, Vec<StructField>>,      // struct 定義
    local_bindings: HashMap<String, LocalRegister>,      // ローカル変数→レジスタ割り当て
    next_free_reg: u8,                       // 次に割り当て可能なレジスタ (V0 から)
    loop_break_offsets: Vec<Vec<ByteOffset>>, // ループの break パッチ先
}

impl CodeGen {
    pub fn new() -> Self { /* ... */ }

    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        // Pass 1: グローバルデータ・関数アドレスを収集
        self.collect_globals(program);
        // Pass 2: 関数本体を生成
        self.generate_functions(program);
        // Pass 3: 前方参照をパッチ
        self.patch_forward_refs();
        self.bytes.clone()  // ROM として出力
    }

    fn generate_expr(&mut self, expr: &Expr) -> ValueLocation {
        // 式を評価し、値の所在を返す（レジスタまたはメモリ）
        match &expr.kind {
            ExprKind::IntLiteral(n) => {
                // LD Vx, kk
                let reg = self.allocate_register();
                self.emit_op(Opcode::LdImm(reg, *n as u8));
                ValueLocation::InRegister(reg)
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let lhs_loc = self.generate_expr(lhs);
                let rhs_loc = self.generate_expr(rhs);
                // op に応じて CHIP-8 命令を生成
                match op {
                    BinOp::Add => {
                        self.emit_op(Opcode::Add(
                            lhs_loc.register().unwrap(),
                            rhs_loc.register().unwrap(),
                        ));
                    }
                    // ... 他の演算子
                }
                ValueLocation::InRegister(lhs_loc.register().unwrap())
            }
            ExprKind::If { cond, then_block, else_block } => {
                // SE / SNE (skip if equal/not equal) + JP で条件分岐
                let cond_reg = self.generate_expr(cond).register().unwrap();
                let skip_offset = self.emit_skip_if(cond_reg);  // SE を発行、パッチ位置を記録
                self.generate_expr(else_block.as_ref().unwrap_or(then_block));
                let jump_offset = self.emit_jp();  // then/else の後へジャンプ
                self.patch_address(skip_offset, ...);  // skip 先をパッチ
            }
            ExprKind::Loop { body } => {
                let loop_addr = self.current_address();
                self.generate_expr(body);
                self.emit_op(Opcode::Jp(loop_addr));  // ループバック
                // break で脱出するオフセットをパッチ
            }
            // ... 他の ExprKind
        }
    }

    fn emit_op(&mut self, op: Opcode) {
        let [byte1, byte2] = op.encode();
        self.bytes.push(byte1);
        self.bytes.push(byte2);
    }

    fn allocate_register(&mut self) -> Register {
        let reg = UserRegister::new(self.next_free_reg);
        self.next_free_reg += 1;
        Register::User(reg)
    }
}
```

#### ValueLocation - 値の所在管理
```rust
enum ValueLocation {
    InRegister(Register),               // V0-VE のどれかに値がある
    InMemory { addr: u16, struct_name: String },  // struct がメモリに配置
    Void,                               // 値を返さない (loop など)
}
```

**コード生成の戦略**:
- **レジスタ割り当て**: ローカル変数ごとに V0-VE の 1 つを割り当て
- **スピル**: テンポラリが足りなくなればスタックに保存
- **前方参照**: 関数呼び出し (CALL) やラベル参照の位置情報を記録し、後で解決
- **パッチング**: 2 パス構成で、最初のパスで関数アドレスを確定，2 番目のパスでそれを使用

**ビギナーのコツ**:
- `emit_op()` でバイトコード配列に命令を追加
- 条件分岐は「skip 命令 + JP」の組み合わせ
- ループは「ループ開始アドレスを記録 → ループバック JP」で実装

---

### 3.5 出力 (Emitter) - `src/emitter/mod.rs` (39行)

**目的**: バイトコード → `.ch8` ファイル

```rust
pub enum EmitError {
    RomTooLarge { size: usize, max: usize },
    IoError(std::io::Error),
}

pub fn emit(bytes: &[u8], output_path: &Path) -> Result<(), EmitError> {
    const MAX_ROM_SIZE: usize = 4096 - 0x200;  // 3584 バイト
    if bytes.len() > MAX_ROM_SIZE {
        return Err(EmitError::RomTooLarge {
            size: bytes.len(),
            max: MAX_ROM_SIZE,
        });
    }
    fs::write(output_path, bytes).map_err(EmitError::IoError)?;
    Ok(())
}
```

**シンプルな設計**:
- CHIP-8 の 4KB メモリ制限（プログラム開始 0x200 = 3584 バイト）をチェック
- バイトコードをそのままバイナリファイルに書き出す

---

## 4. テストが言語規則を表現する

テストは「ドメインルールの表明」です。文字列マッチではなく、**型付きエラーのパターンマッチ**で意味を検証します。

### 4.1 字句解析テスト (`tests/lexer_tests.rs`)

```rust
fn kinds(input: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(input);
    lexer.tokenize().unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn test_hex_number() {
    assert_eq!(
        kinds("0xFF"),
        vec![TokenKind::IntLiteral(0xFF), TokenKind::Eof]
    );
}

#[test]
fn test_keywords() {
    assert_eq!(
        kinds("let fn if else loop break return true false"),
        vec![
            TokenKind::Let,
            TokenKind::Fn,
            // ...
            TokenKind::Eof,
        ]
    );
}
```

**テスト パターン**:
- 数値リテラル（10進、16進 `0x`, 2進 `0b`）
- キーワード vs 識別子の区別
- 演算子・区切り文字
- エラー（不正な文字など）

---

### 4.2 構文解析テスト (`tests/parser_tests.rs`)

```rust
fn parse(input: &str) -> Program {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse_program().unwrap()
}

#[test]
fn test_fn_def_with_params() {
    let prog = parse("fn add(a: u8, b: u8) -> u8 { a + b }");
    match &prog.top_levels[0] {
        TopLevel::FnDef { name, params, return_type, body, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[0].ty, Type::U8);
            // ... body もチェック
        }
        _ => panic!("expected FnDef"),
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
```

**テスト内容**:
- 関数定義（パラメータ・戻り値型）
- let 束縛
- 型（`u8`, `bool`, `sprite(N)`, `[T; N]`）
- 式（リテラル、二項演算、if 式、ループ、関数呼び出し）

---

### 4.3 意味解析テスト (`tests/analyzer_tests.rs`)

**型チェックのテスト**:
```rust
fn analyze(input: &str) -> Result<(), Vec<AnalyzeError>> { /* ... */ }

#[test]
fn test_simple_program() {
    analyze_ok("
        fn main() -> () {
            clear();
        }
    ");
}

#[test]
fn test_undefined_variable() {
    analyze_err_kind(
        "fn f() -> u8 { undefined_var }",
        AnalyzeErrorKind::UndefinedVariable("undefined_var".into()),
    );
}

#[test]
fn test_type_mismatch() {
    analyze_err_matches(
        "fn f() -> u8 { true }",
        |k| matches!(k, AnalyzeErrorKind::TypeMismatch {
            expected: Type::U8,
            found: Type::Bool,
            ..
        }),
    );
}
```

**テスト内容**:
- `main()` の存在確認
- 変数・関数の未定義検出
- 型の不一致
- 関数呼び出しの引数型・個数チェック
- ローカル変数の上限（15 個）
- ループ外での `break`
- match の網羅性

---

### 4.4 コード生成テスト (`tests/codegen_tests.rs`)

```rust
fn compile(input: &str) -> Vec<u8> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut codegen = CodeGen::new();
    codegen.generate(&program)
}

#[test]
fn test_clear_call() {
    let bytes = compile(
        "fn main() -> () {
            clear();
        }",
    );
    // 00E0 (CLS) が含まれる
    let has_cls = bytes.windows(2)
        .any(|w| w[0] == 0x00 && w[1] == 0xE0);
    assert!(has_cls, "expected CLS instruction, got: {:02X?}", bytes);
}

#[test]
fn test_loop_generates_jp() {
    let bytes = compile(
        "fn main() -> () {
            loop {
                break;
            };
        }",
    );
    // JP (1NNN) が複数含まれる
    let jp_count = bytes.chunks(2)
        .filter(|w| w.len() == 2 && (w[0] & 0xF0) == 0x10)
        .count();
    assert!(jp_count >= 2);
}

#[test]
fn test_function_call_generates_call() {
    let bytes = compile(
        "fn helper() -> () { clear(); }
         fn main() -> () { helper(); }",
    );
    // CALL (2NNN) が含まれる
    let has_call = bytes.chunks(2)
        .any(|w| w.len() == 2 && (w[0] & 0xF0) == 0x20);
    assert!(has_call, "expected CALL instruction");
}
```

**テスト内容**:
- 命令の存在確認（バイトコード検査）
- 組み込み関数の命令生成
- 条件分岐のエンコーディング
- ループとジャンプの構造
- 関数呼び出し

---

### 4.5 統合テスト (`tests/integration_tests.rs`)

**フルパイプラインのテスト**:
```rust
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
    assert_eq!(bytes[0] & 0xF0, 0x10);  // 先頭は JP
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
    std::fs::remove_file(&tmp_path).unwrap();
}

#[test]
fn test_design_doc_program_e2e() {
    // DESIGN.md に記載されたプログラムが実際にコンパイルできることを確認
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
}
```

---

## 5. 実装順序とデザイン選択のサンプル

### 5.1 シンプルなプログラムの例

**hello.ch8l** (`examples/hello.ch8l`):
```rust
let digit_sprite: sprite(1) = [0b11110000];

fn main() -> () {
  clear();
  let x: u8 = 28;
  let y: u8 = 14;
  draw(digit_sprite, x, y);
  let k: u8 = wait_key();
}
```

**このプログラムが教えてくれること**:
1. **グローバル変数**: スプライトは定数として定義
2. **組み込み関数**: `clear()`, `draw()`, `wait_key()` の呼び出し
3. **ローカル変数**: `x`, `y`, `k` をレジスタに割り当て
4. **関数エントリーポイント**: `main()` が起動時に呼ばれる

**コンパイル過程**:
```
1. Lexer: digit_sprite, sprite(1), [0b...], clear, () など
2. Parser: LetDef + Sprite型, FnDef + BuiltinCalls → AST
3. Analyzer: 型チェック（digit_sprite: Sprite(1) OK）、clear は組み込み関数 OK
4. CodeGen: 
   - スプライトデータをメモリに配置
   - clear() → 00E0 命令
   - x = 28 → 6X1C (LD Vx, 28)
   - y = 14 → 6Y0E (LD Vy, 14)
   - draw(sprite, x, y) → DXYN 命令
   - wait_key() → FX0A 命令
5. Emitter: バイトコード → hello.ch8
```

---

### 5.2 より複雑なプログラムの例

**integration_tests.rs から**:
```rust
let source = r#"
    let BOARD_W: u8 = 10;
    let BOARD_H: u8 = 20;

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
```

**デザイン選択のポイント**:

| 機能 | 実装方式 | 理由 |
|-----|--------|------|
| **定数定義** | `let BOARD_W: u8 = 10;` （トップレベル） | 静的、最適化可能 |
| **関数定義** | `fn name(params) -> Type { body }` | 明示的な型注釈、型安全性 |
| **条件分岐** | `if cond { then } else { else }` | if 式として値を返す |
| **ループ** | `loop { ... break; }` | 無限ループ + break で脱出制御 |
| **制御フロー** | skip 命令 + JP の組み合わせ | CHIP-8 の特性に合わせた実装 |

---

### 5.3 レジスタ割り当ての例

関数 `draw_block(x, y)` の実行:
```
allocate:
  x → V0
  y → V1

generate:
  LD V0, (xの値)  # 6XKK
  LD V1, (yの値)  # 6YKK
  CALL draw_block # 2NNN

in draw_block:
  LD I, block_sprite_addr  # ANNN
  DRW V0, V1, height       # DXYN
  RET                      # 00EE
```

**制約**: V0-VE (15個) = 最大 10 ローカル変数 + 5 テンポラリ

---

## 6. 既存ドキュメント

### 6.1 CLAUDE.md (`CLAUDE.md`)
- **実装方針**: 型でドメインを表現、enum の網羅性で安全性確保
- **コマンド**: `cargo test`, `cargo clippy`, `cargo doc --open`
- **フェーズ管理**: PR ベースの開発フロー

### 6.2 DESIGN.md (`docs/DESIGN.md`, 207行)
- **言語概要**: 型システム（u8, bool, array, sprite）、構文イメージ
- **コンパイラアーキテクチャ**: パイプラインの詳細図
- **フェーズ別実装ガイド**: Phase 1-5 の具体的タスク
- **CHIP-8 制約**: メモリ、レジスタ、スタック、キーボード

---

## 7. まとめ：ビギナーが学べる内容

| 内容 | ファイル | 重要性 |
|------|---------|--------|
| **型によるドメインモデリング** | `src/chip8.rs`, `src/parser/ast.rs` | ⭐⭐⭐ |
| **Lexer の実装** | `src/lexer/` | ⭐⭐ |
| **再帰下降パーサー** | `src/parser/mod.rs` | ⭐⭐⭐ |
| **型チェック・スコープ** | `src/analyzer/mod.rs` | ⭐⭐⭐ |
| **コード生成・レジスタ割り当て** | `src/codegen/mod.rs` | ⭐⭐⭐ |
| **テスト駆動の言語規則表現** | `tests/` | ⭐⭐ |

### 学習ロードマップ（推奨順序）

1. **ドメイン型の理解** (30分)
   - `src/chip8.rs` の `Register`, `Addr`, `Opcode` を読む
   - `src/parser/ast.rs` の `Type`, `Expr`, `TopLevel` を読む

2. **Lexer の動作確認** (1時間)
   - `src/lexer/mod.rs` と `token.rs` を読む
   - `tests/lexer_tests.rs` を実行し、パターンを理解

3. **Parser の作りを理解** (2時間)
   - `src/parser/mod.rs` の再帰下降パースの流れを読む
   - `tests/parser_tests.rs` で各ルールを確認

4. **Analyzer の型チェックロジック** (2時間)
   - `src/analyzer/mod.rs` の `check_expr()` メソッドを読む
   - `tests/analyzer_tests.rs` で型チェックルールを確認

5. **CodeGen の命令生成** (2時間)
   - `src/codegen/mod.rs` の `generate_expr()` メソッドを読む
   - `tests/codegen_tests.rs` で命令エンコーディングを確認

6. **統合テストで全体を検証** (1時間)
   - `tests/integration_tests.rs` でフルパイプラインを確認
   - `cargo test` で全テストを実行

**合計**: ~8時間で全体像を把握可能

---

## 補足：実行方法

```bash
# テストを全て実行
cargo test

# 特定のテストのみ実行
cargo test lexer_tests
cargo test analyzer_tests::test_simple_program

# ドキュメントを生成・表示
cargo doc --open

# example をコンパイル
cargo run -- examples/hello.ch8l

# フォーマットとリントをチェック
cargo fmt --check
cargo clippy
```

---

