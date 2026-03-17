# chip8-lang: CHIP-8 向け関数型コンパイラ

## プロジェクト概要

CHIP-8 ROM にコンパイルできる小さな関数型言語を Rust で実装する。
学習目的のプロジェクト。詳細な設計は `docs/DESIGN.md` を参照。

## 実装方針

**ソースコードを自己文書化する。** コードのドメインモデルやユースケースは、型・代数的データ型・制約・テストで「意味」として表現する。別途ドキュメントを書くのではなく、コード自体が仕様を語るようにする。

- **型でドメインを表現する**: 生の `u8` や `String` ではなく、`Register`, `Addr`, `Opcode`, `BuiltinFunction` 等の newtype/enum を使い、意味と制約を型レベルで強制する
- **代数的データ型で網羅性を保証する**: enum の exhaustive match により、新しいバリアントの追加時にコンパイラが対応漏れを検出する
- **テストはドメインルールの表明**: テストでは文字列マッチではなく、型付きエラー (`AnalyzeErrorKind` 等) のパターンマッチで意味を検証する
- **stringly-typed を排除する**: 文字列リテラルでの分岐を避け、enum バリアントで型安全に分岐する

## リファレンス

- `cargo doc --open` で生成されるドキュメントがAPIリファレンスとなる
- 特に `src/chip8.rs` の `Opcode` enum と `src/parser/ast.rs` の `BuiltinFunction` enum が言語仕様の中核を型で表現している

## 開発フロー

- main ブランチへの直プッシュは禁止
- Phase ごとにブランチを作成し、PR を通じてマージする
- ブランチ命名: `phase/{番号}-{短い説明}`
- PR マージ前チェック: `cargo test`, `cargo clippy`, `cargo fmt --check`

### Phase ごとの進め方

1. `phase/{番号}-{短い説明}` ブランチを作成
2. 実装を進め、テスト・lint・フォーマットが通ることを確認
3. PR を作成し、ボディに変更内容・テスト計画を記載
4. PR をマージし、次の Phase へ進む

### PR マージまでのワークフロー

```
1. ブランチ作成    git checkout -b phase/{番号}-{短い説明}
2. 実装・テスト    cargo test && cargo clippy && cargo fmt --check
3. コミット        git add <files> && git commit
4. プッシュ        git push -u origin <branch>
5. PR 作成         gh pr create --title "..." --body "..."
6. マージ          gh pr merge <number> --merge --delete-branch
7. ローカル同期    git checkout main && git pull
```

## コマンド

```bash
cargo build          # ビルド
cargo test           # テスト実行
cargo clippy         # lint
cargo fmt --check    # フォーマットチェック
cargo doc --open     # APIリファレンス生成・閲覧
cargo run -- <file>  # コンパイル実行
```

## アーキテクチャ

```
ソースコード (.ch8l) → Lexer → Parser → Analyzer → CodeGen → Emitter → .ch8
```

## ディレクトリ構成

```
src/
  main.rs        # CLI エントリーポイント
  chip8.rs       # CHIP-8 ドメイン型 (Register, Addr, Opcode 等)
  lexer/         # 字句解析
  parser/        # 構文解析 (AST, BuiltinFunction)
  analyzer/      # 意味解析 (型チェック, スコープ解決)
  codegen/       # コード生成
  emitter/       # バイナリ出力
tests/           # テスト
examples/        # サンプルプログラム
docs/            # 設計ドキュメント
```
