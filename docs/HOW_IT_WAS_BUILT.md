# chip8-lang はどう作られたか

この文書は、言語自作の初学者向けに、このリポジトリの言語が**どういう順序と考え方で実装されているか**を読み解くためのレポートです。

結論から言うと、このプロジェクトは「とりあえず文字列を頑張って処理する」のではなく、

- まず **対象マシンである CHIP-8 の型** を定義し、
- 次に **言語の AST と型** を定義し、
- その上で **Lexer → Parser → Analyzer → CodeGen → Emitter** を順に積み上げる

という、かなり正統派の作りになっています。

また、単にコンパイルが通るだけでなく、**型とテストで仕様を表現する**方針がコード全体に一貫しています。

## 0. 最初に見るべき全体像

全体の入口は `src/main.rs` です。

```text
ソースコード (.ch8l)
  -> Lexer
  -> Parser
  -> Analyzer
  -> CodeGen
  -> Emitter
  -> .ch8
```

`main.rs` では本当にこの順番で処理しています。

- `Lexer::new(&source).tokenize()`
- `Parser::new(tokens).parse_program()`
- `Analyzer::new().analyze(&program)`
- `CodeGen::new().generate(&program)`
- `emitter::emit(&bytes, &output_path)`

つまり、このプロジェクトは「CLI から各フェーズを順番に呼ぶ」構造になっていて、初学者が追いやすいです。

補足すると、`Cargo.toml` の `[dependencies]` は空です。つまり、ほぼ標準ライブラリだけで組まれており、「外部ライブラリの魔法」ではなく、コンパイラの基本構造を自力で学ぶための実装になっています。

## 1. まず先に「どんな世界を扱うか」を型で固定している

このプロジェクトで最初に注目したいのは、**コンパイラより前にドメインを型で決めている**ことです。

### CHIP-8 側の型

`src/chip8.rs` には、CHIP-8 の命令生成に必要な型がまとまっています。

- `UserRegister`
- `Register`
- `Addr`
- `SpriteHeight`
- `Opcode`

特に重要なのは `Opcode` です。CHIP-8 命令が `enum` で表現され、`encode()` で 2 バイト列に変換されます。

これは初心者にとって大事なポイントです。  
「コード生成器がいきなり生の `u16` を組み立てる」のではなく、まず **意味のある命令の列** を作り、最後にエンコードしています。

この設計には次の利点があります。

- 命令の種類が `enum` で列挙されるので、対応漏れが起きにくい
- `Addr` や `Register` によって不正な値を混ぜにくい
- `chip8.rs` 自体のユニットテストで命令エンコードを独立に検証できる

つまり「バックエンドの土台」を先に固めているわけです。

### 言語側の型

言語の構造は `src/parser/ast.rs` にあります。

- `Type`
- `Expr` / `ExprKind`
- `Stmt` / `StmtKind`
- `TopLevel`
- `Program`
- `BuiltinFunction`

ここを見ると、この言語は当初の設計書 `docs/DESIGN.md` よりも実装が先に進んでいることが分かります。設計書には主に `let`、`fn`、`if`、`loop` が書かれていますが、実装済み AST にはさらに次が入っています。

- `match`
- `enum`
- `struct`
- フィールドアクセス
- 配列アクセス
- パイプ演算子 `|>`
- `random_enum`

初学者向けに言い換えると、このプロジェクトは「最初に最小言語を作って終わり」ではなく、**AST を中心に機能を段階的に増やせる形** で作られています。

## 2. Lexer は「小さく、でも位置情報をちゃんと持つ」

字句解析は `src/lexer/mod.rs`、トークン定義は `src/lexer/token.rs` にあります。

ここでの設計はかなり教科書的です。

- `TokenKind` にキーワード・演算子・区切り文字・リテラルを列挙
- `Token` に `kind` と `span` を持たせる
- `Span` で行・列を保持する

この `span` があるおかげで、エラー表示は `line:column: message` の形になります。

Lexer の実装で初心者が学びやすい点は次の 3 つです。

### 2-1. 数値リテラルを基数ごとに分けて読む

`read_number()` から

- `read_decimal_number()`
- `read_hex_number()`
- `read_binary_number()`

へ分岐しています。

`0x` や `0b` を見て処理を分ける作りなので、「字句解析器は巨大な if 文ではなく、小さな読み取り関数の集合に分解できる」という良い例になっています。

### 2-2. コメントと空白を先に吸う

`next_token()` で、

- 空白を飛ばす
- `--` コメントを飛ばす
- その後でトークンを読む

という順序を取っています。

字句解析で大事なのは「意味のあるトークンを返す前に、ノイズを片付ける」ことですが、その手順が素直に実装されています。

### 2-3. 機能追加時に TokenKind を増やせばよい

`match`、`enum`、`struct`、`mut`、`::`、`=>`、`.`、`..`、`|>` が `TokenKind` に増えています。

これは重要で、新機能を追加するときはまず Lexer のトークンに表れます。  
このリポジトリはその流れがとても見やすいです。

## 3. Parser は再帰下降 + Pratt parsing

構文解析は `src/parser/mod.rs` にあります。

この Parser は二段構えです。

- 文法の大枠は **再帰下降**
- 式の優先順位は **Pratt parsing**

### 3-1. トップレベルは再帰下降で素直に読む

`parse_program()` は `parse_top_level()` を繰り返し呼びます。  
`parse_top_level()` は次の 4 種類を分岐します。

- `fn`
- `let`
- `enum`
- `struct`

ここは初学者にとってとても読みやすいです。  
「今のトークンを見て、どの構文か決める」という再帰下降パーサの基本が、そのまま見えます。

### 3-2. 型構文も AST に直結している

`parse_type()` は

- `u8`
- `bool`
- `()`
- `[T; N]`
- `sprite(N)`
- ユーザー定義型

を `Type` に変換します。

つまり、このプロジェクトでは「型注釈の構文」も最初から AST と一体で設計されています。  
後段の意味解析が楽なのは、この段階で `Type` がはっきり作られているからです。

### 3-3. 式は Pratt parsing で優先順位を処理

`parse_expr()` は `parse_expr_bp()` を呼びます。  
これが Pratt parser の本体で、`+`, `*`, `==`, `&&` などの優先順位を binding power で処理しています。

初心者にとっての学びは、「式だけは専用の仕組みを使うと楽」という点です。  
再帰下降だけで優先順位を全部手書きするより、かなり見通しが良くなります。

### 3-4. パース段階で軽い脱糖もしている

例えば `parse_primary()` では、関数名が組み込み関数なら `ExprKind::BuiltinCall` に変換しています。

また `|>` は `parse_pipe_rhs()` によって通常の関数呼び出しに近い形へ落とし込まれます。  
つまり Parser は単に木を作るだけでなく、**後段が扱いやすい AST へ少し整形する役目** も持っています。

### 3-5. 機能追加の跡がよく見える

`parse_primary()` を見ると、

- `Name::Variant` は enum variant
- `Name { ... }` は struct literal
- `name(...)` は call / builtin call
- `{ ... }` は block
- `if`, `match`, `loop`

のように分岐しています。

これは「新機能を追加するとき、Parser のどこを触るか」の見本として非常に良いです。  
AST にバリアントを足し、Lexer にトークンを足し、Parser に分岐を足す、という言語実装の基本サイクルがそのまま見えます。

## 4. Analyzer は「型チェック」だけでなく「CHIP-8 制約の翻訳」もしている

意味解析は `src/analyzer/mod.rs` です。

このモジュールはとても重要です。  
単なる型検査器ではなく、**高水準言語のルールを CHIP-8 の制約に接続する役目** を持っています。

### 4-1. まず定義を全部集める 2 パス構成

`Analyzer::analyze()` は次の流れです。

1. Pass 1 でトップレベル定義を登録
2. `main` の存在を確認
3. `UserType` が本当に存在するか確認
4. Pass 2 で各定義の本体を型チェック

この 2 パス構成はとても実践的です。  
関数や enum や struct を先に表に載せてから本体を検査するので、前方参照や相互参照に対応しやすくなります。

### 4-2. エラーの型がかなり豊富

`AnalyzeErrorKind` には、初学者が「意味解析で何を検査するのか」を学ぶ材料が詰まっています。

- `MissingMain`
- `UndefinedVariable`
- `UndefinedFunction`
- `TypeMismatch`
- `BuiltinArgCountMismatch`
- `IfConditionNotBool`
- `TooManyLocals`
- `MatchScrutineeType`
- `NonExhaustiveMatch`
- `UndefinedStruct`
- `MissingFields`
- `ImmutableAssignment`

重要なのは、テストが文字列ではなく **この enum をパターンマッチして検証している** ことです。  
つまり「エラーメッセージ」ではなく「意味上どんな失敗だったか」を仕様として固定しています。

### 4-3. CHIP-8 のレジスタ制約が analyzer に入っている

定数を見ると、

- `USER_REGISTER_COUNT = 15`
- `MAX_TEMP_REGISTERS = 5`
- `MAX_LOCALS = 10`

となっています。

これは CHIP-8 の `V0`〜`VE` を使えるレジスタとして見なし、そのうち一部をテンポラリ用に予約している、という意味です。  
つまり Analyzer は「型が合っているか」だけでなく、**この言語を CHIP-8 に載せたとき現実的に実装できるか** まで見ています。

`tests/analyzer_tests.rs` の `test_too_many_locals()` や `test_struct_locals_dont_count_toward_register_limit()` は、その制約が仕様として固定されている例です。

### 4-4. `enum` と `match` で網羅性まで見る

`ExprKind::Match` に対して Analyzer は、

- scrutinee が `u8` または enum か
- arm が空でないか
- pattern がリテラルか enum variant か
- arm の戻り値型が揃っているか
- enum match が網羅的か

を確認しています。

`test_match_enum_exhaustive()` と `test_match_enum_non_exhaustive()` を見ると、「網羅性チェック」が実際の仕様として入っていることが分かります。

### 4-5. `struct` は「型」だけでなく「表現方法」を意識している

Analyzer 側でも `struct` は特別扱いされています。  
`struct` 型ローカルはレジスタ数制限の計算から外しています。

これは後段の CodeGen が「struct はメモリに置く」という戦略を取っているからです。  
つまり Analyzer と CodeGen は別モジュールですが、**同じ実装方針を共有** しています。

## 5. CodeGen は「素直な命令生成」だけでなく、かなり実装上の工夫がある

コード生成は `src/codegen/mod.rs` です。  
このファイルはこのプロジェクトの中で一番「コンパイラを作っている感じ」が強い場所です。

### 5-1. CodeGen も多段パスになっている

`CodeGen::generate()` は次の順です。

1. Pass 0: enum / struct 定義を登録
2. Pass 1: グローバル定数やスプライトをデータセクションへ集める
3. Pass 2: 関数本体のコードを生成
4. 最後に前方参照を解決し、`bytes + data` を結合する

ここで分かるのは、この言語は「全部 1 回の走査で吐く」設計ではなく、**あとで必要になる情報を先に集める** 設計だということです。

### 5-2. 先頭に `JP main` を置き、後でパッチする

`generate()` の最初に `emit_placeholder()` でダミー命令を置き、関数アドレスが確定してから `patch_at(..., Opcode::Jp(main_addr))` しています。

これはコンパイラ実装で非常によく出る「前方参照の解決」です。  
関数の位置がまだ分からない段階では本物のジャンプ先を書けないので、あとから書き換えています。

### 5-3. struct はレジスタではなくメモリに逃がす

この CodeGen の大きな特徴は、`ValueLocation` と `LocalBinding` で

- スカラー値はレジスタ
- struct 値はメモリ

と分けていることです。

これは非常に良い設計判断です。  
CHIP-8 のレジスタは 16 本しかないため、複合データを全部レジスタで持つのは苦しいです。そこで struct をメモリ表現に逃がし、スカラーだけを主にレジスタで扱っています。

この方針のおかげで、

- struct 引数
- struct 戻り値
- ネストした struct
- struct update 構文
- struct 同値比較

まで実装できています。

実際、`tests/codegen_tests.rs` には

- `test_struct_param_memory_backed()`
- `test_struct_return_to_memory()`
- `test_nested_struct_field_access_memory()`
- `test_struct_update_memory()`

があり、メモリベースの実装がコード生成の中心になっていると分かります。

### 5-4. 命令選択は AST の意味に対応している

分かりやすい例として、テストには次があります。

- `test_sprite_and_draw()` は `ANNN` と `DXYN` を確認
- `test_match_generates_se_jp()` は `match` が `SE + JP` 系のパターンになることを確認
- `test_function_call_generates_call()` は `CALL`
- `test_random_enum_generates_rnd()` は `RND`

つまり CodeGen は「高水準構文を CHIP-8 命令の組み合わせへ落とす」という、バックエンドの本質をストレートに見せてくれます。

### 5-5. Tail Call Optimization まで入っている

`codegen_expr_tail()` では、末尾位置の自己再帰を検出して `CALL` ではなく `JP` に変換しています。

これはかなり面白い点です。  
小さな言語処理系でも、**AST 上で「末尾位置かどうか」を区別すれば最適化ができる** ことを示しています。

`test_tco_self_recursion_generates_jp()` と `test_non_tail_recursion_generates_call()` は、その違いをきれいに検証しています。

### 5-6. レジスタ保護のための現実的な工夫がある

`emit_global_read()` や `emit_store_to_memory()` には、`V0` を壊さないための XOR swap パターンが入っています。  
これは「理論的には簡単」では済まない、実機制約を相手にしたコード生成らしい工夫です。

初学者にとって大切なのは、ここから

> 高級言語の意味を保ちながら、少ないレジスタで値を安全にやりくりする必要がある

という、コード生成の本当の難しさが見えてくることです。

## 6. Emitter は小さいが、最後の責務を明確に分けている

`src/emitter/mod.rs` は短いですが、役割分担のお手本です。

- `CodeGen` はバイト列を作る
- `Emitter` はそれを `.ch8` ファイルへ書く

しかも、ROM サイズ上限 `4096 - 0x200` を超えたら `EmitError::RomTooLarge` を返します。

この分離は重要です。  
「コード生成」と「ファイル出力」を別責務にしているので、`tests/integration_tests.rs` では

- バイト列が作れるか
- 実際にファイルへ書けるか
- サイズ制限で失敗するか

を独立にテストできます。

## 7. テストは単なる確認ではなく、言語仕様そのもの

このプロジェクトを初学者に特に勧めたい理由は、`tests/` が非常に読みやすいことです。

### 7-1. フェーズごとにテストが分かれている

- `tests/lexer_tests.rs`
- `tests/parser_tests.rs`
- `tests/analyzer_tests.rs`
- `tests/codegen_tests.rs`
- `tests/integration_tests.rs`

この構成だけで、コンパイラの各段階が独立した責務を持つことが分かります。

### 7-2. Analyzer のテストが「意味」を守っている

例えば `analyzer_tests.rs` では、`AnalyzeErrorKind` を直接検証しています。

これは非常に良い書き方です。  
エラーメッセージ文字列に依存すると、文章を少し直しただけでテストが壊れます。しかしこのプロジェクトでは、「何が失敗だったか」という意味を enum で固定しています。

### 7-3. CodeGen のテストが命令パターンを見ている

`codegen_tests.rs` では、生成バイト列に

- `CALL`
- `JP`
- `SE`
- `RND`
- `DRW`

が含まれるかを見ています。

これは初学者にとってすごく参考になります。  
「コード生成テストは完全一致だけが正解ではない。意味のある命令パターンを確認する方法もある」と分かるからです。

### 7-4. 統合テストがパイプライン全体を保証する

`tests/integration_tests.rs` の `compile_to_bytes()` は、

`Lexer -> Parser -> Analyzer -> CodeGen`

をそのまま通しています。

そして `test_full_pipeline()`、`test_hello_example_compiles()`、`test_design_doc_program_e2e()` が、全体として言語が成立していることを確認しています。

## 8. 設計書から実装へ、どう育ったか

`docs/DESIGN.md` は出発点としてとても良い文書です。  
ただし、現在の実装はそこからさらに発展しています。

設計書で中心だったもの:

- `let`
- `fn`
- `if`
- `loop`
- 組み込み関数

実装でさらに増えたもの:

- `enum`
- `match`
- `struct`
- 可変グローバル
- パイプ演算子
- `random_enum`
- struct をメモリ配置する実装戦略
- 自己再帰の TCO

つまり、この言語は

1. まず最小パイプラインを作る
2. AST と型を拡張しやすい形にしておく
3. テストを追加しながら機能を増やす

という形で育ったと読むのが自然です。

## 9. 初学者がこのコードを追うなら、どの順で読むと良いか

おすすめの読み順は次です。

1. `src/main.rs`  
   まず全体の流れを掴む。

2. `src/parser/ast.rs`  
   この言語が何を表現できるかを知る。

3. `src/chip8.rs`  
   ターゲットマシンの命令と制約を知る。

4. `src/lexer/token.rs` と `src/lexer/mod.rs`  
   文字列がどうトークンになるかを見る。

5. `src/parser/mod.rs`  
   トークン列がどう AST になるかを見る。

6. `src/analyzer/mod.rs`  
   AST にどんな意味ルールを課しているかを見る。

7. `src/codegen/mod.rs`  
   AST をどう CHIP-8 命令へ落としているかを見る。

8. `tests/`  
   仕様をテストがどう表現しているか確認する。

最初から `codegen/mod.rs` に突っ込むと大変ですが、この順番ならかなり追いやすいです。

## 10. この実装から学べること

このプロジェクトから学べる本質は、次の 4 つです。

### 10-1. 言語機能は AST に現れる

新機能を作るときは、多くの場合

1. Token を増やす
2. AST を増やす
3. Parser を増やす
4. Analyzer を増やす
5. CodeGen を増やす
6. テストを増やす

という順で進みます。

### 10-2. 型を先に作ると後段が楽になる

`Type`、`BuiltinFunction`、`Opcode` が早い段階で型になっているため、後続処理が stringly-typed になっていません。  
これは小さい実装でもかなり効きます。

### 10-3. 意味解析は型チェックだけではない

このプロジェクトの Analyzer は、

- 型チェック
- スコープ解決
- `main` の存在確認
- `match` の網羅性
- CHIP-8 レジスタ制約の検証

まで担当しています。

つまり意味解析は「文法の後片付け」ではなく、**高水準言語の約束を実行可能性へ接続する場所** です。

### 10-4. テストは仕様書になれる

とくに `analyzer_tests.rs` と `codegen_tests.rs` は、言語仕様を読む入口として優秀です。  
初学者がこのコードベースを学ぶなら、実装とテストを往復しながら読むのが一番理解しやすいでしょう。

## まとめ

`chip8-lang` は、

- 小さな対象マシンを相手にしつつ、
- AST・型・意味解析・コード生成をきちんと分離し、
- テストで仕様を固定しながら育てられた

とても良い学習用コンパイラです。

特に良いのは、**「まず動かす」だけで終わらず、型とテストで設計を支えていること** です。  
言語自作の初学者が「どこから実装すればよいのか」「機能追加のたびに何を変えるのか」を学ぶには、かなり教材向きのコードベースだと言えます。

最初の一歩としては、`examples/hello.ch8l` をコンパイルし、その後 `tests/parser_tests.rs` と `tests/analyzer_tests.rs` を読みながら `src/parser/ast.rs` を行き来するのがおすすめです。
