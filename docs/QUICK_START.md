# クイックスタート：CHIP-8 言語処理系の実装概要

## 📊 プロジェクト構成

```
chip8-lang/
├── src/
│   ├── lib.rs              (19行) - 6モジュールの公開インターフェース
│   ├── main.rs             (76行) - CLI エントリーポイント、パイプラインの実行
│   ├── chip8.rs           (360行) - CHIP-8 ドメイン型（Register, Addr, Opcode）
│   ├── lexer/
│   │   ├── mod.rs         (397行) - 字句解析ロジック
│   │   └── token.rs        (81行) - トークン型定義
│   ├── parser/
│   │   ├── mod.rs         (871行) - 再帰下降パーサー
│   │   └── ast.rs         (272行) - AST 型定義
│   ├── analyzer/
│   │   └── mod.rs        (1056行) - 型チェック・スコープ解決
│   ├── codegen/
│   │   └── mod.rs        (1623行) - CHIP-8 バイトコード生成
│   └── emitter/
│       └── mod.rs          (39行) - .ch8 ファイル出力
├── tests/
│   ├── lexer_tests.rs          - 字句解析テスト
│   ├── parser_tests.rs         - 構文解析テスト
│   ├── analyzer_tests.rs       - 型チェックテスト
│   ├── codegen_tests.rs        - コード生成テスト
│   ├── integration_tests.rs    - フルパイプラインテスト
│   └── chip8_interpreter.rs    - CHIP-8 エミュレータ（テスト用）
├── examples/
│   └── hello.ch8l         - 最小サンプル
├── docs/
│   ├── DESIGN.md          - 設計ドキュメント
│   └── IMPLEMENTATION_GUIDE.md - ビギナー向け詳細解説
└── Cargo.toml
```

## 🔄 コンパイルパイプライン

```
ソースコード (.ch8l)
    ↓
┌──────────┐
│  Lexer   │ src/lexer/ - 文字列 → トークン
└──────────┘
    ↓
┌──────────┐
│  Parser  │ src/parser/ - トークン → AST
└──────────┘
    ↓
┌──────────┐
│ Analyzer │ src/analyzer/ - 型チェック・スコープ解決
└──────────┘
    ↓
┌──────────┐
│ CodeGen  │ src/codegen/ - AST → CHIP-8 バイトコード
└──────────┘
    ↓
┌──────────┐
│ Emitter  │ src/emitter/ - バイトコード → .ch8 ROM
└──────────┘
```

## 🎯 核となる3つの型システム

### 1. CHIP-8 ドメイン型 (`src/chip8.rs`)
**メモリ・レジスタの物理的制約を型で表現**

```rust
pub struct UserRegister(u8);      // V0-VE のみ（14個）
pub enum Register {
    User(UserRegister),
    Flag,                         // VF は予約
}
pub struct Addr(u16);             // 12bit アドレス
pub enum Opcode { Cls, Ret, Jp, Call, ... }  // ~35個の命令
```

### 2. 言語 AST 型 (`src/parser/ast.rs`)
**プログラム構造を代数的データ型で表現**

```rust
pub enum Type { U8, Bool, Unit, Array(...), Sprite(...), UserType(...) }
pub enum Expr { IntLiteral, Ident, BinaryOp, If, Loop, Block, ... }
pub enum BuiltinFunction { Clear, Draw, WaitKey, Random, ... }
```

### 3. エラー型（各モジュール）
**ドメインルール違反を型付きで報告**

```rust
pub enum LexErrorKind { InvalidNumber, UnexpectedCharacter, ... }
pub enum ParseErrorKind { UnexpectedToken, UnknownType, ... }
pub enum AnalyzeErrorKind { MissingMain, TypeMismatch, TooManyLocals, ... }
```

## 📝 実装順序（推奨）

1. **ドメイン型を理解する** (30分)
   - `src/chip8.rs` を読む - CHIP-8 物理制約の型化
   - `src/parser/ast.rs` を読む - 言語構文の型化

2. **Lexer を実装・テストする** (1-2時間)
   - `src/lexer/token.rs` - トークン型
   - `src/lexer/mod.rs` - 文字→トークン変換ロジック
   - `tests/lexer_tests.rs` でテスト

3. **Parser を実装・テストする** (2-3時間)
   - 再帰下降パーサーの基本
   - 演算子優先度（Pratt parsing）
   - `tests/parser_tests.rs` でテスト

4. **Analyzer を実装・テストする** (2-3時間)
   - 型チェック（各式の型を判定）
   - スコープ管理（グローバル vs ローカル）
   - CHIP-8 制約チェック（ローカル変数 15個上限など）
   - `tests/analyzer_tests.rs` でテスト

5. **CodeGen を実装・テストする** (3-4時間)
   - レジスタ割り当て
   - 命令生成（加算なら ADD, 条件なら SE/SNE + JP など）
   - `tests/codegen_tests.rs` でテスト

6. **Emitter で統合・テストする** (30分-1時間)
   - サイズチェック（4KB 上限）
   - ファイル出力
   - `tests/integration_tests.rs` でフルパイプライン確認

## 🏗️ 各モジュールの設計パターン

| モジュール | 入出力 | 主要構造体 | キーメソッド |
|----------|--------|----------|------------|
| **Lexer** | &str → Vec\<Token\> | `Lexer { input, pos, ... }` | `tokenize()` |
| **Parser** | Vec\<Token\> → AST | `Parser { tokens, pos }` | `parse_program()` |
| **Analyzer** | AST → AST (+ 型情報) | `Analyzer { globals, locals, ... }` | `analyze()` |
| **CodeGen** | AST → Vec\<u8\> | `CodeGen { bytes, fn_addrs, ... }` | `generate()` |
| **Emitter** | Vec\<u8\> → File | なし (関数型) | `emit()` |

## 🧪 テスト戦略

### ユニットテスト（各モジュール）
- **Lexer**: 数値リテラル、キーワード、演算子
- **Parser**: 関数定義、let、if/loop、型注釈
- **Analyzer**: 型チェック、undefined エラー、オーバーフロー検出
- **CodeGen**: 命令エンコーディング、レジスタ割り当て

### 統合テスト
- フルパイプラインで実行可能な ROM を生成
- ファイル出力と読み込みの確認
- 設計ドキュメントのサンプルが実際に動作すること

## 🚀 実行方法

```bash
# テスト実行
cargo test                          # 全テスト
cargo test lexer_tests              # 字句解析テストのみ
cargo test analyzer_tests::test_type_mismatch  # 特定テスト

# APIドキュメント生成
cargo doc --open

# example コンパイル
cargo run -- examples/hello.ch8l
# → examples/hello.ch8 が生成される

# コード品質チェック
cargo clippy
cargo fmt --check
```

## 📚 推奨読む順序

1. **docs/DESIGN.md** - 言語仕様・全体概観
2. **src/chip8.rs** + **src/parser/ast.rs** - ドメイン型
3. **src/main.rs** - パイプラインの流れ
4. **各モジュールのテスト** (`tests/` 配下)
5. **docs/IMPLEMENTATION_GUIDE.md** - 深い詳細解説

## 💡 学習ポイント

✅ **型によるドメインモデリング**
- 生の u8 ではなく Register, Addr, Opcode 型を使用
- 型システムがコンパイラの助言役になる

✅ **代数的データ型と網羅性**
- enum の match で全ケースを網羅
- 新しいバリアント追加時にコンパイラが指摘

✅ **多段パイプライン**
- 各フェーズが独立した入出力を持つ
- エラーハンドリングの位置づけ

✅ **テストが仕様の表現**
- 文字列マッチではなく型付きエラーでテスト
- テストコードがドメインルールの明示

✅ **制約のある環境へのコンパイル**
- 限定されたリソース（15レジスタ）の効率的割り当て
- アーキテクチャ固有の制約をどう型にするか

## 🔗 ファイルサイズ（全体 4794行）

```
codegen     1623行 (34%) - 最大モジュール
analyzer    1056行 (22%)
parser       871行 (18%)
lexer        397行 ( 8%)
chip8        360行 ( 8%)
ast          272行 ( 6%)
main          76行 ( 2%)
emitter       39行 ( 1%)
```

コード生成とアナライザで全コードの 56% を占める（複雑度が高い）

---

**すべての詳細は docs/IMPLEMENTATION_GUIDE.md を参照してください。**
