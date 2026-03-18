use chip8_lang::codegen::CodeGen;
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;

mod chip8_interpreter;

fn compile(input: &str) -> Vec<u8> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut codegen = CodeGen::new();
    codegen.generate(&program)
}

fn compile_and_run(input: &str) -> u8 {
    let bytes = compile(input);
    let mut vm = chip8_interpreter::Chip8::new(&bytes);
    vm.run_and_get_v0().expect("program hanged (cycle limit)")
}

#[test]
fn test_empty_main() {
    let bytes = compile("fn main() -> () { }");
    // JP main (1NNN) + main body (JP self = halt)
    assert!(bytes.len() >= 4);
    // 最初の命令は JP (1xxx)
    assert_eq!(bytes[0] & 0xF0, 0x10);
    // main の末尾はセルフループ (JP to self) で停止
    let last_two = &bytes[bytes.len() - 2..];
    assert_eq!(
        last_two[0] & 0xF0,
        0x10,
        "expected JP instruction at end of main"
    );
    // ジャンプ先アドレスが命令自体のアドレスと一致する
    let jump_target = ((last_two[0] as u16 & 0x0F) << 8) | last_two[1] as u16;
    let instruction_addr = 0x200 + (bytes.len() as u16 - 2);
    assert_eq!(jump_target, instruction_addr, "expected self-loop halt");
}

#[test]
fn test_main_halts_without_stack_underflow() {
    // main が値を返す有限プログラムがセルフループで正常停止する
    let result = compile_and_run("fn main() -> u8 { 42 }");
    assert_eq!(result, 42);
}

#[test]
fn test_clear_call() {
    let bytes = compile(
        "fn main() -> () {
            clear();
        }",
    );
    // 00E0 (CLS) が含まれる
    let has_cls = bytes.windows(2).any(|w| w[0] == 0x00 && w[1] == 0xE0);
    assert!(has_cls, "expected CLS instruction, got: {:02X?}", bytes);
}

#[test]
fn test_set_delay() {
    let bytes = compile(
        "fn main() -> () {
            set_delay(60);
        }",
    );
    // 6XKK (LD Vx, 60 = 0x3C) が含まれる
    let has_ld = bytes
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0x60 && w[1] == 60);
    assert!(has_ld, "expected LD Vx, 60, got: {:02X?}", bytes);
    // FX15 (set_delay) が含まれる
    let has_set_delay = bytes
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xF0 && w[1] == 0x15);
    assert!(
        has_set_delay,
        "expected FX15 instruction, got: {:02X?}",
        bytes
    );
}

#[test]
fn test_wait_key() {
    let bytes = compile(
        "fn main() -> () {
            let k: u8 = wait_key();
        }",
    );
    // FX0A (wait_key) が含まれる
    let has_wait = bytes
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xF0 && w[1] == 0x0A);
    assert!(has_wait, "expected FX0A instruction, got: {:02X?}", bytes);
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
    // JP (1NNN) が含まれる (ループバック)
    let jp_count = bytes
        .chunks(2)
        .filter(|w| w.len() == 2 && (w[0] & 0xF0) == 0x10)
        .count();
    // 少なくとも2つのJP: main への JP + ループバック JP + break JP
    assert!(
        jp_count >= 2,
        "expected at least 2 JP instructions, got {}, bytes: {:02X?}",
        jp_count,
        bytes
    );
}

#[test]
fn test_function_call_generates_call() {
    let bytes = compile(
        "fn helper() -> () { clear(); }
         fn main() -> () { helper(); }",
    );
    // CALL (2NNN) が含まれる
    let has_call = bytes
        .chunks(2)
        .any(|w| w.len() == 2 && (w[0] & 0xF0) == 0x20);
    assert!(has_call, "expected CALL instruction, got: {:02X?}", bytes);
}

#[test]
fn test_sprite_and_draw() {
    let bytes = compile(
        "let s: sprite(1) = [0b11110000];
         fn main() -> () {
            let x: u8 = 10;
            let y: u8 = 5;
            draw(s, x, y);
        }",
    );
    // ANNN (LD I, addr) が含まれる
    let has_ld_i = bytes
        .chunks(2)
        .any(|w| w.len() == 2 && (w[0] & 0xF0) == 0xA0);
    assert!(has_ld_i, "expected ANNN instruction, got: {:02X?}", bytes);
    // DXYN (DRW) が含まれる
    let has_drw = bytes
        .chunks(2)
        .any(|w| w.len() == 2 && (w[0] & 0xF0) == 0xD0);
    assert!(has_drw, "expected DXYN instruction, got: {:02X?}", bytes);
}

#[test]
fn test_if_generates_skip_and_jp() {
    let bytes = compile(
        "fn main() -> () {
            let x: u8 = 5;
            if x == 5 {
                clear();
            };
        }",
    );
    // 命令が生成されていることを確認
    assert!(bytes.len() > 4);
}

#[test]
fn test_binary_add() {
    let bytes = compile(
        "fn main() -> () {
            let x: u8 = 3;
            let y: u8 = 4;
            let z: u8 = x + y;
        }",
    );
    // 8XY4 (ADD Vx, Vy) が含まれる
    let has_add = bytes
        .chunks(2)
        .any(|w| w.len() == 2 && (w[0] & 0xF0) == 0x80 && (w[1] & 0x0F) == 0x04);
    assert!(has_add, "expected ADD instruction, got: {:02X?}", bytes);
}

#[test]
fn test_output_starts_with_jp() {
    let bytes = compile("fn main() -> () { }");
    // プログラムは必ず JP main から始まる
    assert_eq!(bytes[0] & 0xF0, 0x10, "first instruction should be JP");
}

#[test]
fn test_design_doc_program() {
    let bytes = compile(
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
    // コンパイルが成功し、バイトコードが生成される
    assert!(bytes.len() > 10, "expected substantial bytecode output");
    // JP main
    assert_eq!(bytes[0] & 0xF0, 0x10);
}

#[test]
fn test_match_generates_se_jp() {
    let bytes = compile(
        "fn main() -> u8 {
            let x: u8 = 1;
            match x {
                0 => 10,
                1 => 20,
                2 => 30,
            }
        }",
    );
    // match はSE+JP パターンを生成するはず
    assert!(bytes.len() > 10);
    // SE (3xxx) が含まれること
    assert!(
        bytes.chunks(2).any(|c| c[0] & 0xF0 == 0x30),
        "expected SE instruction in match codegen"
    );
}

#[test]
fn test_enum_variant_generates_ld_imm() {
    let bytes = compile(
        "enum Dir { Up, Down, Left }
         fn main() -> u8 {
            let d: Dir = Dir::Down;
            match d {
                Dir::Up => 0,
                Dir::Down => 1,
                Dir::Left => 2,
            }
         }",
    );
    assert!(bytes.len() > 4);
}

#[test]
fn test_function_call_saves_registers() {
    let bytes = compile(
        "fn add_one(x: u8) -> u8 {
            x + 1
        }
        fn main() -> u8 {
            let a: u8 = 5;
            let b: u8 = add_one(a);
            a + b
        }",
    );
    // FX55 (LD [I], Vx) と FX65 (LD Vx, [I]) が含まれるはず
    let has_fx55 = bytes.chunks(2).any(|c| c[1] == 0x55 && c[0] & 0xF0 == 0xF0);
    let has_fx65 = bytes.chunks(2).any(|c| c[1] == 0x65 && c[0] & 0xF0 == 0xF0);
    assert!(has_fx55, "expected FX55 (register save) instruction");
    assert!(has_fx65, "expected FX65 (register restore) instruction");
}

#[test]
fn test_pipe_compiles() {
    let bytes = compile(
        "fn double(x: u8) -> u8 { x + x }
         fn main() -> u8 { 5 |> double() }",
    );
    // パイプはパース時に脱糖されるので、通常の関数呼び出しと同じバイトコード
    assert!(bytes.len() > 4);
    // CALL (2NNN) が含まれること
    assert!(
        bytes.chunks(2).any(|c| c[0] & 0xF0 == 0x20),
        "expected CALL instruction"
    );
}

#[test]
fn test_function_body_result_copied_to_v0() {
    // draw() は VF を返す。関数本体の結果が VF の場合、RET 前に LD V0, VF が必要
    let bytes = compile(
        "let sprite: sprite(1) = [0b11110000];
         fn check(x: u8, y: u8) -> bool {
            draw(sprite, x, y)
         }
         fn main() -> () {
            let c: bool = check(0, 0);
         }",
    );
    // check 関数内に LD V0, VF (8F0x パターンではなく 80F0) が含まれるはず
    // 8XY0 = LD Vx, Vy → 80F0 = LD V0, VF
    let has_ld_v0_vf = bytes.chunks(2).any(|c| c[0] == 0x80 && c[1] == 0xF0);
    assert!(
        has_ld_v0_vf,
        "expected LD V0, VF before RET in function returning draw result"
    );
}

#[test]
fn test_random_enum_generates_rnd() {
    let bytes = compile(
        "enum Piece { I, O, T, S, Z, L, J }
         fn main() -> Piece { random_enum(Piece) }",
    );
    // RND 命令 (CXKK) が含まれること
    let has_rnd = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0xC0);
    assert!(has_rnd, "expected RND instruction for random_enum");
}

#[test]
fn test_random_enum_power_of_two() {
    // 4 バリアント (2の冪) → mask = 3, 拒否サンプリング不要
    let bytes = compile(
        "enum Dir { Up, Down, Left, Right }
         fn main() -> Dir { random_enum(Dir) }",
    );
    let has_rnd = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0xC0);
    assert!(has_rnd, "expected RND instruction for random_enum");
    // RND Vx, 0x03 (mask = 3) が含まれるはず
    let has_rnd_mask3 = bytes
        .chunks(2)
        .any(|c| (c[0] & 0xF0) == 0xC0 && c[1] == 0x03);
    assert!(
        has_rnd_mask3,
        "expected RND with mask 0x03 for 4-variant enum"
    );
}

#[test]
fn test_tco_self_recursion_generates_jp() {
    // 末尾再帰は CALL ではなく JP にコンパイルされるべき
    let bytes = compile(
        "fn count(n: u8) -> u8 {
            if n == 0 { 0 } else { count(n - 1) }
         }
         fn main() -> u8 { count(5) }",
    );
    // count 関数内の末尾再帰は JP (1NNN) であるべき
    // main 関数からの count 呼び出しは CALL (2NNN) であるべき
    // JP は最初の命令 (main へ) と count 内の TCO とループ的なものがある
    let jp_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x10).count();
    let call_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x20).count();
    // CALL は main→count の1回のみ (TCO 分は JP に変換)
    assert_eq!(call_count, 1, "expected exactly 1 CALL (main→count)");
    // JP は: main へのジャンプ + if-else の条件ジャンプ + TCO ジャンプ (最低3つ)
    assert!(
        jp_count >= 3,
        "expected at least 3 JP instructions (main + if-else + TCO)"
    );
}

#[test]
fn test_non_tail_recursion_generates_call() {
    // 非末尾位置の再帰は通常の CALL のまま
    let bytes = compile(
        "fn add_one(n: u8) -> u8 {
            if n == 0 { 0 } else { add_one(n - 1) + 1 }
         }
         fn main() -> u8 { add_one(3) }",
    );
    // add_one(n-1) + 1 は末尾位置ではない → CALL が2つ (main→add_one, add_one→add_one)
    let call_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x20).count();
    assert_eq!(
        call_count, 2,
        "expected 2 CALL instructions (non-tail recursion)"
    );
}

#[test]
fn test_struct_literal_and_field_access() {
    // struct リテラルとフィールドアクセスがコンパイルできること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn main() -> u8 {
            let p: Pos = Pos { x: 10, y: 20 };
            p.x
         }",
    );
    // コンパイルが成功し、適切なバイトコードが生成されること
    assert!(bytes.len() >= 4);
    // LD 命令 (6XKK) が含まれること (10 と 20 のロード)
    let has_ld_10 = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x60 && c[1] == 10);
    let has_ld_20 = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x60 && c[1] == 20);
    assert!(has_ld_10, "expected LD with value 10");
    assert!(has_ld_20, "expected LD with value 20");
}

#[test]
fn test_enum_equality_compiles() {
    let bytes = compile(
        "enum Dir { Up, Down }
         fn main() -> bool {
            let d: Dir = Dir::Up;
            d == Dir::Down
         }",
    );
    // SE (5XY0) 命令が含まれること (等値比較)
    let has_se = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x50);
    assert!(has_se, "expected SE instruction for enum equality");
}

#[test]
fn test_struct_equality_compiles() {
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn main() -> bool {
            let a: Pos = Pos { x: 1, y: 2 };
            let b: Pos = Pos { x: 1, y: 2 };
            a == b
         }",
    );
    // SE (5XY0) が2つ以上含まれること (フィールドごとの比較)
    let se_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x50).count();
    assert!(
        se_count >= 2,
        "expected at least 2 SE instructions for struct equality (field-by-field)"
    );
}

#[test]
fn test_struct_param_memory_backed() {
    // struct パラメータをメモリ経由で受け渡しできること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn get_x(p: Pos) -> u8 { p.x }
         fn main() -> u8 {
            let p: Pos = Pos { x: 5, y: 10 };
            get_x(p)
         }",
    );
    assert!(bytes.len() >= 4);
    // CALL 命令が含まれること
    let has_call = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x20);
    assert!(has_call, "expected CALL instruction");
}

#[test]
fn test_multiple_struct_params_no_overflow() {
    // 複数の struct 引数でもレジスタオーバーフローしないこと
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn add_pos(a: Pos, b: Pos) -> u8 {
            a.x + b.x
         }
         fn main() -> u8 {
            let p1: Pos = Pos { x: 1, y: 2 };
            let p2: Pos = Pos { x: 3, y: 4 };
            add_pos(p1, p2)
         }",
    );
    assert!(bytes.len() >= 4);
    let has_call = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x20);
    assert!(has_call, "expected CALL instruction");
}

#[test]
fn test_struct_return_to_memory() {
    // struct を返す関数の戻り値がメモリ経由で使えること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn make_pos(x: u8, y: u8) -> Pos {
            Pos { x: x, y: y }
         }
         fn main() -> u8 {
            let p: Pos = make_pos(10, 20);
            p.x
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_nested_struct_field_access_memory() {
    // ネストした struct のフィールドアクセスが動作すること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         struct Entity { pos: Pos, hp: u8 }
         fn get_entity_x(e: Entity) -> u8 {
            e.pos.x
         }
         fn main() -> u8 {
            let e: Entity = Entity { pos: Pos { x: 42, y: 10 }, hp: 100 };
            get_entity_x(e)
         }",
    );
    assert!(bytes.len() >= 4);
    // 42 がロードされること
    let has_ld_42 = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x60 && c[1] == 42);
    assert!(has_ld_42, "expected LD with value 42");
}

#[test]
fn test_struct_equality_memory_backed() {
    // メモリ上の struct 同士の等値比較が動作すること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn same_pos(a: Pos, b: Pos) -> bool {
            a == b
         }
         fn main() -> bool {
            let p1: Pos = Pos { x: 1, y: 2 };
            let p2: Pos = Pos { x: 1, y: 2 };
            same_pos(p1, p2)
         }",
    );
    assert!(bytes.len() >= 4);
    // SE 命令 (5XY0) が含まれること (フィールドごとの比較)
    let se_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x50).count();
    assert!(
        se_count >= 2,
        "expected at least 2 SE instructions for struct equality"
    );
}

#[test]
fn test_tco_with_struct_param() {
    // TCO + struct 引数が動作すること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn move_right(p: Pos, n: u8) -> u8 {
            if n == 0 {
               p.x
            } else {
               move_right(Pos { x: p.x + 1, y: p.y }, n - 1)
            }
         }
         fn main() -> u8 {
            move_right(Pos { x: 0, y: 0 }, 5)
         }",
    );
    assert!(bytes.len() >= 4);
    // JP 命令が含まれること (TCO)
    let has_jp = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x10);
    assert!(has_jp, "expected JP instruction for TCO");
}

#[test]
fn test_tco_after_struct_let_does_not_leak_temps() {
    let bytes = compile(
        "struct GameState { a: u8, b: u8, c: u8, d: u8, e: u8, f: u8 }
         fn next_state(state: GameState) -> GameState {
             state
         }
         fn loop_state(state: GameState, n: u8) -> u8 {
             if n == 0 {
                state.a
             } else {
                let next: GameState = next_state(state);
                loop_state(next, n - 1)
             }
         }
         fn main() -> u8 {
             loop_state(GameState { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6 }, 3)
         }",
    );
    assert!(bytes.len() >= 4);
    let has_jp = bytes.chunks(2).any(|c| (c[0] & 0xF0) == 0x10);
    assert!(has_jp, "expected JP instruction for TCO after struct let");
}

#[test]
fn test_issue34_multiple_struct_args() {
    // issue #34 の再現ケース: 複数の struct 引数 + ローカル変数
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn distance_x(a: Pos, b: Pos) -> u8 {
            let dx: u8 = b.x - a.x;
            dx
         }
         fn main() -> u8 {
            let p1: Pos = Pos { x: 10, y: 20 };
            let p2: Pos = Pos { x: 30, y: 40 };
            distance_x(p1, p2)
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_struct_update_memory() {
    // struct update 構文がメモリベースで動作すること
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn move_x(p: Pos) -> Pos {
            Pos { x: p.x + 1, ..p }
         }
         fn main() -> u8 {
            let p: Pos = Pos { x: 5, y: 10 };
            let q: Pos = move_x(p);
            q.x
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_if_else_returns_struct_from_memory() {
    // #40: if-else で struct (InMemory) を返す場合のバグ修正
    let bytes = compile(
        "struct Pos { x: u8, y: u8 }
         fn choose(c: bool, a: Pos, b: Pos) -> Pos {
            if c { a } else { b }
         }
         fn main() -> u8 {
            let p: Pos = choose(true, Pos { x: 1, y: 2 }, Pos { x: 3, y: 4 });
            p.x
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_mul_compiles() {
    let bytes = compile(
        "fn main() -> u8 {
            let a: u8 = 3;
            let b: u8 = 4;
            a * b
         }",
    );
    assert!(bytes.len() >= 4);
    // JP 命令が含まれること (ループ)
    let jp_count = bytes.chunks(2).filter(|c| (c[0] & 0xF0) == 0x10).count();
    assert!(jp_count >= 2, "expected JP instructions for mul loop");
}

#[test]
fn test_div_compiles() {
    let bytes = compile(
        "fn main() -> u8 {
            let a: u8 = 12;
            let b: u8 = 3;
            a / b
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_mod_compiles() {
    let bytes = compile(
        "fn main() -> u8 {
            let a: u8 = 10;
            let b: u8 = 3;
            a % b
         }",
    );
    assert!(bytes.len() >= 4);
}

#[test]
fn test_mutable_global_read_write() {
    let bytes = compile(
        "let mut score: u8 = 0;
         fn add_score(points: u8) -> () {
            score = score + points;
         }
         fn main() -> () {
            add_score(10);
         }",
    );
    assert!(bytes.len() >= 4);
    // FX55 (LdIVx) が含まれること (グローバル書き込み)
    let has_fx55 = bytes.chunks(2).any(|c| (c[1] & 0xFF) == 0x55);
    assert!(has_fx55, "expected FX55 instruction for global write");
}

#[test]
fn test_array_index_assign_compiles() {
    let bytes = compile(
        "let mut board: [u8; 4] = [0, 0, 0, 0];
         fn main() -> () {
            board[2] = 42;
         }",
    );
    assert!(bytes.len() >= 4);
    // FX1E (AddI) が含まれること (インデックス計算)
    let has_fx1e = bytes.chunks(2).any(|c| (c[1] & 0xFF) == 0x1E);
    assert!(has_fx1e, "expected FX1E instruction for index calculation");
}

// ============================================================
// CHIP-8 インタプリタによる実行検証テスト
// ============================================================

#[test]
fn test_run_simple_return() {
    assert_eq!(compile_and_run("fn main() -> u8 { 42 }"), 42);
}

#[test]
fn test_run_flat_struct_field_access() {
    assert_eq!(
        compile_and_run(
            "struct Big { a: u8, b: u8, c: u8, d: u8, e: u8 }
             fn get_e(s: Big) -> u8 { s.e }
             fn main() -> u8 {
                get_e(Big { a: 1, b: 3, c: 5, d: 7, e: 9 })
             }"
        ),
        9
    );
}

#[test]
fn test_run_flat_struct_first_field() {
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             fn get_x(p: Pos) -> u8 { p.x }
             fn main() -> u8 {
                get_x(Pos { x: 42, y: 10 })
             }"
        ),
        42
    );
}

#[test]
fn test_run_issue42_nested_struct_speed() {
    // Issue #42: ネスト struct のスカラーフィールド (nested struct の後ろ)
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_speed(s: GameState) -> u8 { s.speed }
             fn main() -> u8 {
                get_speed(GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 })
             }"
        ),
        9
    );
}

#[test]
fn test_run_issue42_nested_struct_pos_x() {
    // Issue #42: ネスト struct のフィールドアクセス
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_px(s: GameState) -> u8 { s.pos.x }
             fn main() -> u8 {
                get_px(GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 })
             }"
        ),
        2
    );
}

#[test]
fn test_run_issue42_nested_struct_pos_y() {
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_py(s: GameState) -> u8 { s.pos.y }
             fn main() -> u8 {
                get_py(GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 })
             }"
        ),
        3
    );
}

#[test]
fn test_run_issue42_nested_struct_score() {
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_score(s: GameState) -> u8 { s.score }
             fn main() -> u8 {
                get_score(GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 })
             }"
        ),
        8
    );
}

#[test]
fn test_run_issue42_nested_struct_piece() {
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_piece(s: GameState) -> u8 { s.piece }
             fn main() -> u8 {
                get_piece(GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 })
             }"
        ),
        1
    );
}

#[test]
fn test_run_issue42_nested_struct_with_let() {
    // let バインド経由でネスト struct を渡す
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_speed(s: GameState) -> u8 { s.speed }
             fn main() -> u8 {
                let gs: GameState = GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 };
                get_speed(gs)
             }"
        ),
        9
    );
}

#[test]
fn test_run_issue42_nested_struct_let_and_local() {
    // let バインド + 他のローカル変数がある場合
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_speed(s: GameState) -> u8 { s.speed }
             fn main() -> u8 {
                let x: u8 = 100;
                let gs: GameState = GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 };
                get_speed(gs)
             }"
        ),
        9
    );
}

#[test]
fn test_run_issue42_multiple_nested_calls() {
    // 複数のフィールドアクセスを別々の関数で
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn get_speed(s: GameState) -> u8 { s.speed }
             fn get_score(s: GameState) -> u8 { s.score }
             fn main() -> u8 {
                let gs: GameState = GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 8, speed: 9 };
                let sp: u8 = get_speed(gs);
                let sc: u8 = get_score(gs);
                sp + sc
             }"
        ),
        17
    );
}

#[test]
fn test_run_issue42_deeply_nested() {
    // 3 段ネスト
    assert_eq!(
        compile_and_run(
            "struct Inner { val: u8 }
             struct Mid { inner: Inner, tag: u8 }
             struct Outer { mid: Mid, extra: u8 }
             fn get_val(o: Outer) -> u8 { o.mid.inner.val }
             fn get_extra(o: Outer) -> u8 { o.extra }
             fn main() -> u8 {
                let o: Outer = Outer { mid: Mid { inner: Inner { val: 77 }, tag: 88 }, extra: 99 };
                get_val(o) + get_extra(o)
             }"
        ),
        176 // 77 + 99
    );
}

#[test]
fn test_run_issue42_deeply_nested_val_only() {
    // 3 段ネストで inner.val だけ取得
    assert_eq!(
        compile_and_run(
            "struct Inner { val: u8 }
             struct Mid { inner: Inner, tag: u8 }
             struct Outer { mid: Mid, extra: u8 }
             fn get_val(o: Outer) -> u8 { o.mid.inner.val }
             fn main() -> u8 {
                get_val(Outer { mid: Mid { inner: Inner { val: 77 }, tag: 88 }, extra: 99 })
             }"
        ),
        77
    );
}

#[test]
fn test_run_issue42_deeply_nested_tag() {
    assert_eq!(
        compile_and_run(
            "struct Inner { val: u8 }
             struct Mid { inner: Inner, tag: u8 }
             struct Outer { mid: Mid, extra: u8 }
             fn get_tag(o: Outer) -> u8 { o.mid.tag }
             fn main() -> u8 {
                get_tag(Outer { mid: Mid { inner: Inner { val: 77 }, tag: 88 }, extra: 99 })
             }"
        ),
        88
    );
}

#[test]
fn test_run_issue42_deeply_nested_extra() {
    assert_eq!(
        compile_and_run(
            "struct Inner { val: u8 }
             struct Mid { inner: Inner, tag: u8 }
             struct Outer { mid: Mid, extra: u8 }
             fn get_extra(o: Outer) -> u8 { o.extra }
             fn main() -> u8 {
                get_extra(Outer { mid: Mid { inner: Inner { val: 77 }, tag: 88 }, extra: 99 })
             }"
        ),
        99
    );
}

#[test]
fn test_run_issue42_mid_struct_access() {
    // 2 段ネスト: Mid { inner: Inner, tag } 直接
    assert_eq!(
        compile_and_run(
            "struct Inner { val: u8 }
             struct Mid { inner: Inner, tag: u8 }
             fn get_tag(m: Mid) -> u8 { m.tag }
             fn main() -> u8 {
                get_tag(Mid { inner: Inner { val: 77 }, tag: 88 })
             }"
        ),
        88
    );
}

// ===== Issue #44: struct フィールドアクセスで値が壊れるバグ =====

#[test]
fn test_run_issue44_struct_first_field_passed_correctly() {
    // V0 クロバリングの回帰テスト: local_var_count=0 で struct を渡すとき
    // field0 (V0) が後続フィールドのロードで破壊されないこと
    assert_eq!(
        compile_and_run(
            "struct Big { a: u8, b: u8, c: u8, d: u8, e: u8 }
             fn get_a(s: Big) -> u8 { s.a }
             fn main() -> u8 {
                get_a(Big { a: 42, b: 2, c: 3, d: 4, e: 5 })
             }"
        ),
        42
    );
}

#[test]
fn test_run_issue44_struct_pass_no_local_vars() {
    // local_var_count=0 で 2 フィールド struct の先頭フィールドが正しいか
    assert_eq!(
        compile_and_run(
            "struct Pair { x: u8, y: u8 }
             fn first(p: Pair) -> u8 { p.x }
             fn main() -> u8 {
                first(Pair { x: 7, y: 99 })
             }"
        ),
        7
    );
}

#[test]
fn test_run_issue44_multiple_calls_with_struct_fields() {
    // issue #44 パターン: 複数関数呼び出し + struct フィールドアクセス
    assert_eq!(
        compile_and_run(
            "struct State { speed: u8, score: u8, level: u8 }
             fn use_score(s: u8) -> u8 { s }
             fn make_state(sp: u8, sc: u8, lv: u8) -> State {
                State { speed: sp, score: sc, level: lv }
             }
             fn on_land(st: State) -> State {
                use_score(st.score);
                let x: u8 = 5;
                make_state(st.speed, st.score + 1, x)
             }
             fn main() -> u8 {
                let s: State = State { speed: 10, score: 20, level: 30 };
                let s2: State = on_land(s);
                s2.speed
             }"
        ),
        10
    );
}

#[test]
fn test_run_issue44_struct_all_fields_after_pass() {
    // struct 全フィールドが関数呼び出し後も正しいことを確認
    assert_eq!(
        compile_and_run(
            "struct Triple { a: u8, b: u8, c: u8 }
             fn sum_fields(t: Triple) -> u8 { t.a + t.b + t.c }
             fn main() -> u8 {
                sum_fields(Triple { a: 10, b: 20, c: 30 })
             }"
        ),
        60
    );
}

// ===== Issue #46: ネスト struct コピーがレジスタを破壊するバグ =====

#[test]
fn test_run_issue46_nested_struct_literal_preserves_registers() {
    // ネスト struct (Pos) のコピーで V0/V1 が破壊されないこと
    // バグだと score = 0 になる (V1 が Pos.y = 0 で上書きされる)
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn make_gs(p: u8, sc: u8, sp: u8) -> GameState {
                GameState { piece: p, pos: Pos { x: 28, y: 0 }, score: sc, speed: sp }
             }
             fn main() -> u8 {
                let gs: GameState = make_gs(1, 6, 15);
                gs.score
             }"
        ),
        6
    );
}

#[test]
fn test_run_issue46_on_land_pattern() {
    // issue #46 の再現パターン: update_score 後に struct フィールドが壊れない
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn use_val(v: u8) -> u8 { v }
             fn make_gs(p: u8, sc: u8, sp: u8) -> GameState {
                GameState { piece: p, pos: Pos { x: 28, y: 0 }, score: sc, speed: sp }
             }
             fn on_land(state: GameState) -> GameState {
                use_val(state.score);
                let p: u8 = 7;
                make_gs(p, state.score + 1, state.speed)
             }
             fn main() -> u8 {
                let s: GameState = GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 5, speed: 15 };
                let r: GameState = on_land(s);
                r.score
             }"
        ),
        6
    );
}

#[test]
fn test_run_issue46_struct_update_syntax_preserves_registers() {
    // struct update syntax (base) でレジスタが壊れないこと
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn update(gs: GameState) -> GameState {
                GameState { ..gs, score: gs.score + 1 }
             }
             fn main() -> u8 {
                let s: GameState = GameState { piece: 1, pos: Pos { x: 2, y: 3 }, score: 10, speed: 20 };
                let r: GameState = update(s);
                r.score
             }"
        ),
        11
    );
}

#[test]
fn test_run_issue46_nested_struct_piece_preserved() {
    // ネスト struct コピー後に piece (V0) が壊れないこと
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             struct GameState { piece: u8, pos: Pos, score: u8, speed: u8 }
             fn make_gs(p: u8, sc: u8, sp: u8) -> GameState {
                GameState { piece: p, pos: Pos { x: 28, y: 0 }, score: sc, speed: sp }
             }
             fn main() -> u8 {
                let gs: GameState = make_gs(42, 6, 15);
                gs.piece
             }"
        ),
        42
    );
}

// ---- issue #56: リーフ関数 caller-save 最適化 ----

#[test]
fn test_leaf_function_fewer_saves() {
    // リーフ関数呼び出しでは caller-save が最適化され、
    // 非リーフ関数より少ない FX55/FX65 が生成されること
    let leaf_bytes = compile(
        "fn add_one(x: u8) -> u8 { x + 1 }
         fn main() -> u8 {
            let a: u8 = 1;
            let b: u8 = 2;
            let c: u8 = 3;
            let d: u8 = 4;
            let e: u8 = 5;
            add_one(a) + b + c + d + e
         }",
    );
    let non_leaf_bytes = compile(
        "fn identity(x: u8) -> u8 { x }
         fn add_one(x: u8) -> u8 { identity(x) + 1 }
         fn main() -> u8 {
            let a: u8 = 1;
            let b: u8 = 2;
            let c: u8 = 3;
            let d: u8 = 4;
            let e: u8 = 5;
            add_one(a) + b + c + d + e
         }",
    );

    // FX55 (レジスタ退避) の最大レジスタ番号を比較
    // リーフ呼び出しでは退避範囲が小さいはず
    let leaf_max_save = leaf_bytes
        .chunks(2)
        .filter(|c| c[1] == 0x55 && c[0] & 0xF0 == 0xF0)
        .map(|c| c[0] & 0x0F)
        .max()
        .unwrap_or(0);
    let non_leaf_max_save = non_leaf_bytes
        .chunks(2)
        .filter(|c| c[1] == 0x55 && c[0] & 0xF0 == 0xF0)
        .map(|c| c[0] & 0x0F)
        .max()
        .unwrap_or(0);
    assert!(
        leaf_max_save < non_leaf_max_save,
        "leaf caller-save should use fewer registers: leaf={}, non_leaf={}",
        leaf_max_save,
        non_leaf_max_save
    );
}

#[test]
fn test_leaf_optimization_correctness() {
    // リーフ関数最適化で計算結果が正しいことを検証
    assert_eq!(
        compile_and_run(
            "fn add_one(x: u8) -> u8 { x + 1 }
             fn main() -> u8 {
                let a: u8 = 10;
                let b: u8 = 20;
                let c: u8 = add_one(a);
                a + b + c
             }"
        ),
        41 // 10 + 20 + 11 = 41
    );
}

#[test]
fn test_leaf_optimization_multiple_calls() {
    // 複数のリーフ関数呼び出しで各変数が保護されること
    assert_eq!(
        compile_and_run(
            "fn double(x: u8) -> u8 { x + x }
             fn main() -> u8 {
                let a: u8 = 3;
                let b: u8 = 5;
                let c: u8 = double(a);
                let d: u8 = double(b);
                c + d
             }"
        ),
        16 // 6 + 10 = 16
    );
}

#[test]
fn test_leaf_optimization_with_struct_param() {
    // struct パラメータを持つリーフ関数でも正しく動作すること
    assert_eq!(
        compile_and_run(
            "struct Pos { x: u8, y: u8 }
             fn sum_pos(p: Pos) -> u8 { p.x + p.y }
             fn main() -> u8 {
                let a: u8 = 100;
                let p: Pos = Pos { x: 10, y: 20 };
                let s: u8 = sum_pos(p);
                s + a
             }"
        ),
        130 // 30 + 100 = 130
    );
}

#[test]
fn test_non_leaf_still_saves_all() {
    // 非リーフ関数呼び出しでは全レジスタが退避されること
    assert_eq!(
        compile_and_run(
            "fn identity(x: u8) -> u8 { x }
             fn add_via_identity(x: u8) -> u8 { identity(x) + 1 }
             fn main() -> u8 {
                let a: u8 = 10;
                let b: u8 = 20;
                let c: u8 = add_via_identity(a);
                a + b + c
             }"
        ),
        41 // 10 + 20 + 11 = 41
    );
}
