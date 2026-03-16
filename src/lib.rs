//! chip8-lang: CHIP-8 ROM にコンパイルする関数型言語
//!
//! コンパイルパイプライン:
//! ```text
//! ソースコード (.ch8l) → Lexer → Parser → Analyzer → CodeGen → Emitter → .ch8
//! ```

/// 意味解析 (型チェック・スコープ解決)
pub mod analyzer;
/// CHIP-8 ハードウェアのドメイン型 (レジスタ・アドレス・命令セット)
pub mod chip8;
/// AST から CHIP-8 バイトコードへのコード生成
pub mod codegen;
/// バイトコードを ROM ファイルとして出力
pub mod emitter;
/// ソースコードからトークン列への字句解析
pub mod lexer;
/// トークン列から AST への構文解析
pub mod parser;
