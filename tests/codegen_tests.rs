use chip8_lang::codegen::CodeGen;
use chip8_lang::lexer::Lexer;
use chip8_lang::parser::Parser;

fn compile(input: &str) -> Vec<u8> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    let mut codegen = CodeGen::new();
    codegen.generate(&program)
}

#[test]
fn test_empty_main() {
    let bytes = compile("fn main() -> () { }");
    // JP main (1NNN) + main body (RET = 00EE)
    assert!(bytes.len() >= 4);
    // 最初の命令は JP (1xxx)
    assert_eq!(bytes[0] & 0xF0, 0x10);
    // main の RET (00EE)
    assert!(bytes.contains(&0x00));
    assert!(bytes.contains(&0xEE));
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
fn test_match_generates_sne_jp() {
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
    // match はSNE+JP パターンを生成するはず
    assert!(bytes.len() > 10);
    // SNE (4xxx) が含まれること
    assert!(
        bytes.chunks(2).any(|c| c[0] & 0xF0 == 0x40),
        "expected SNE instruction in match codegen"
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
