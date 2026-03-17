# CHIP-8 言語処理系：ドキュメント索引

このディレクトリには、CHIP-8 向け関数型言語コンパイラの設計・実装に関するドキュメントが含まれています。

## 📚 ドキュメント一覧

### 1. **QUICK_START.md** (最初に読む！)
**分量**: 219行 | **所要時間**: 15分
- プロジェクト全体の構成図
- 6つの主要モジュール一覧表
- コンパイルパイプラインのビジュアル化
- 3つの核となる型システムの概要
- 推奨実装順序（時間見積もり付き）
- 各モジュールの設計パターン表
- 学習ポイント 5つ

**対象者**: 初心者向け、全体像を素早く把握したい人

---

### 2. **DESIGN.md** (言語仕様書)
**分量**: 206行 | **所要時間**: 30分
- 言語の設計方針
- 型システム (u8, bool, array, sprite)
- 構文イメージ（サンプルコード付き）
- 組み込み関数一覧（CHIP-8 命令へのマッピング）
- コンパイラアーキテクチャの詳細図
- 5 フェーズの実装計画
- CHIP-8 ハードウェア制約メモ

**対象者**: 言語仕様を理解したい人、実装前の仕様確認

---

### 3. **IMPLEMENTATION_GUIDE.md** (ビギナー向け詳細解説)
**分量**: 1008行 | **所要時間**: 2-3時間
- コンパイラパイプライン全体解説
- **6つモジュール別の深い実装解説**:
  - CHIP-8 ドメイン型 (Register, Addr, Opcode)
  - 言語 AST 型 (Type, Expr, BuiltinFunction)
  - Lexer の実装パターン
  - 再帰下降パーサーの仕組み
  - Analyzer の型チェックロジック
  - CodeGen のレジスタ割り当て戦略
  - Emitter の簡潔な実装
- **各フェーズのテスト方法**:
  - 字句解析テスト (lexer_tests.rs)
  - 構文解析テスト (parser_tests.rs)
  - 型チェックテスト (analyzer_tests.rs)
  - コード生成テスト (codegen_tests.rs)
  - 統合テスト (integration_tests.rs)
- **実装順序とデザイン選択の具体例**
- **コード例と型安全性の利点**

**対象者**: ビギナー向け詳細解説、モジュール毎に学びたい人

---

### 4. **HOW_IT_WAS_BUILT.md** (工業的背景)
**分量**: 545行 | **所要時間**: 45分
- 実装方針の背景：「型でドメインを表現する」
- なぜ型安全性が重要か
- stringly-typed コードの排除
- テスト戦略：テストが言語規則の表明
- Phase ベースの開発フロー
- コマンド一覧と実行方法
- 推奨学習ロードマップ

**対象者**: 実装哲学・開発プロセスを理解したい人

---

## 🎯 推奨読む順序

### パターン A: 全体を素早く把握したい（1時間）
1. **QUICK_START.md** - 全体構成を図で理解
2. **DESIGN.md** - 言語仕様確認
3. `cargo test` で動作を体験

### パターン B: 実装からの学習（4-6時間）
1. **QUICK_START.md** - プロジェクト構成把握
2. `src/chip8.rs` + `src/parser/ast.rs` - ドメイン型を読む
3. **IMPLEMENTATION_GUIDE.md** - モジュール毎の詳細学習
4. 各 `tests/` ファイルでテストを読み込む
5. `cargo test` でテスト実行

### パターン C: 深い理解＋カスタマイズ（8-10時間）
1. **QUICK_START.md** - 概要把握
2. **HOW_IT_WAS_BUILT.md** - 実装哲学学習
3. **DESIGN.md** - 仕様確認
4. **IMPLEMENTATION_GUIDE.md** - 全モジュール学習
5. ソースコードの詳細実装を読む
6. テストコードで動作パターン学習
7. 自分で拡張機能を実装・テスト

---

## 📊 ドキュメント相互関係図

```
QUICK_START.md (全体図)
    ↓
    ├─→ DESIGN.md (言語仕様)
    │       └─→ ソースコード詳読
    │
    └─→ IMPLEMENTATION_GUIDE.md (詳細解説)
            ├─→ 各モジュール実装
            ├─→ テストコード読み込み
            └─→ HOW_IT_WAS_BUILT.md (背景と開発フロー)
```

---

## 🔗 ファイル参照マップ

### 型システムについて学ぶ
- **入門**: QUICK_START.md § 「核となる3つの型システム」
- **詳細**: IMPLEMENTATION_GUIDE.md § 2.1 / 2.2
- **ソース**: 
  - `src/chip8.rs` (360行) - CHIP-8 ドメイン型
  - `src/parser/ast.rs` (272行) - 言語 AST 型

### Lexer を学ぶ
- **概要**: QUICK_START.md § 「各モジュールの設計パターン」
- **詳細**: IMPLEMENTATION_GUIDE.md § 3.1
- **テスト**: `tests/lexer_tests.rs`
- **ソース**: 
  - `src/lexer/token.rs` (81行)
  - `src/lexer/mod.rs` (397行)

### Parser を学ぶ
- **概要**: DESIGN.md § 「Phase 2: Parser」
- **詳細**: IMPLEMENTATION_GUIDE.md § 3.2
- **テスト**: `tests/parser_tests.rs`
- **ソース**: 
  - `src/parser/ast.rs` (272行)
  - `src/parser/mod.rs` (871行)

### Analyzer を学ぶ
- **概要**: DESIGN.md § 「Phase 3: 意味解析」
- **詳細**: IMPLEMENTATION_GUIDE.md § 3.3
- **テスト**: `tests/analyzer_tests.rs`
- **ソース**: `src/analyzer/mod.rs` (1056行)

### CodeGen を学ぶ
- **概要**: DESIGN.md § 「Phase 4: コード生成」
- **詳細**: IMPLEMENTATION_GUIDE.md § 3.4
- **テスト**: `tests/codegen_tests.rs`
- **ソース**: `src/codegen/mod.rs` (1623行)

### 統合テストとパイプライン
- **詳細**: IMPLEMENTATION_GUIDE.md § 4.5
- **テスト**: `tests/integration_tests.rs`
- **エントリー**: `src/main.rs` (76行)

### 実装哲学と開発プロセス
- **詳細**: HOW_IT_WAS_BUILT.md

---

## 💻 実行コマンド

```bash
# 全テスト実行
cargo test

# 特定テストのみ
cargo test lexer_tests
cargo test parser_tests
cargo test analyzer_tests
cargo test codegen_tests
cargo test integration_tests

# APIドキュメント生成
cargo doc --open

# サンプルコンパイル
cargo run -- examples/hello.ch8l

# コード品質チェック
cargo clippy
cargo fmt --check
```

---

## 📈 コード統計

| モジュール | 行数 | 比率 | 複雑度 |
|----------|------|------|--------|
| codegen  | 1623 | 34% | ⭐⭐⭐⭐ |
| analyzer | 1056 | 22% | ⭐⭐⭐⭐ |
| parser   | 871  | 18% | ⭐⭐⭐ |
| lexer    | 397  | 8%  | ⭐⭐ |
| chip8    | 360  | 8%  | ⭐⭐ |
| ast      | 272  | 6%  | ⭐⭐ |
| main     | 76   | 2%  | ⭐ |
| emitter  | 39   | 1%  | ⭐ |
| **合計** | **4794** | **100%** | |

---

## 🎓 学習目標チェックリスト

実装を進めるにあたり、以下の項目が理解できていることを確認してください：

### Level 1: 基礎理解
- [ ] 6つのモジュールと役割が説明できる
- [ ] コンパイルパイプラインの流れが図説できる
- [ ] Register, Addr, Opcode などドメイン型の意義が理解できている

### Level 2: 各モジュール理解
- [ ] Lexer がトークンを生成する仕組みが理解できている
- [ ] 再帰下降パーサーの優先度処理が理解できている
- [ ] 型チェックがどう動作するか説明できる
- [ ] レジスタ割り当てのロジックが理解できている

### Level 3: 統合理解
- [ ] ソースコードから ROM への全フローが説明できる
- [ ] テストが言語規則の表明であることが理解できている
- [ ] CHIP-8 制約がコード生成にどう影響するか説明できる
- [ ] 型安全性がなぜ重要かが理解できている

### Level 4: 応用・拡張
- [ ] 新しい型を追加できる
- [ ] 新しい組み込み関数を実装できる
- [ ] エラー型を拡張できる
- [ ] 新しいテストを書いて検証できる

---

## 🤔 よくある質問

**Q: 全部読む必要ある？**  
A: いいえ。QUICK_START.md と DESIGN.md だけでも十分に始められます。

**Q: どのドキュメントが最も重要？**  
A: IMPLEMENTATION_GUIDE.md が最も詳しい解説です。ただし QUICK_START.md で全体像をつかんでからの方が効果的です。

**Q: ソースコードはいつ読む？**  
A: IMPLEMENTATION_GUIDE.md の該当セクションで「ファイル参照」が示されている箇所を、そのタイミングで読むのが効率的です。

**Q: テストは重要？**  
A: 非常に重要です。テストが言語規則の明示になっているので、テストを読むことで仕様がわかります。

---

## 📞 ドキュメント更新履歴

| 日付 | ドキュメント | 変更内容 |
|------|---------|--------|
| 2024-03-17 | IMPLEMENTATION_GUIDE.md | 初版作成（1008行） |
| 2024-03-17 | QUICK_START.md | 初版作成（219行） |
| 2024-03-17 | HOW_IT_WAS_BUILT.md | 初版作成（545行） |
| 2024-03-17 | README.md | このファイル作成 |

---

**Happy learning! 🎉**

このドキュメントで不足している部分があれば、対応するソースコードのコメントを参照してください。
また、`cargo doc --open` で生成される API ドキュメントも有用です。
