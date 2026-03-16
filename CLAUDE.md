# chip8-lang: CHIP-8 向け関数型コンパイラ

## プロジェクト概要

CHIP-8 ROM にコンパイルできる小さな関数型言語を Rust で実装する。
学習目的のプロジェクト。詳細な設計は `docs/DESIGN.md` を参照。

## 開発フロー

- main ブランチへの直プッシュは禁止
- Phase ごとにブランチを作成し、PR を通じてマージする
- ブランチ命名: `phase/{番号}-{短い説明}`
- PR マージ前チェック: `cargo test`, `cargo clippy`, `cargo fmt --check`

## コマンド

```bash
cargo build          # ビルド
cargo test           # テスト実行
cargo clippy         # lint
cargo fmt --check    # フォーマットチェック
cargo run -- <file>  # コンパイル実行 (将来)
```

## アーキテクチャ

```
ソースコード (.ch8l) → Lexer → Parser → Analyzer → CodeGen → Emitter → .ch8
```

## ディレクトリ構成

```
src/
  main.rs        # CLI エントリーポイント
  lexer/         # 字句解析
  parser/        # 構文解析
  analyzer/      # 意味解析
  codegen/       # コード生成
  emitter/       # バイナリ出力
tests/           # テスト
examples/        # サンプルプログラム
docs/            # 設計ドキュメント
```
