use std::collections::HashMap;

use crate::parser::ast::*;

/// CHIP-8 プログラムの開始アドレス
const PROGRAM_START: u16 = 0x200;

/// コード生成器
pub struct CodeGen {
    /// 生成されたバイトコード
    bytes: Vec<u8>,
    /// データセクション (スプライトなど)
    data: Vec<u8>,
    /// 関数名 → アドレスのマッピング
    fn_addrs: HashMap<String, u16>,
    /// グローバル変数名 → データアドレスのマッピング
    global_addrs: HashMap<String, u16>,
    /// グローバル変数のスプライトサイズ
    sprite_sizes: HashMap<String, usize>,
    /// ローカル変数名 → レジスタ番号
    local_regs: HashMap<String, u8>,
    /// 次に使えるレジスタ番号
    next_reg: u8,
    /// ローカル変数にバインドされたレジスタの上限 (temp レジスタリセット用)
    bound_reg_count: u8,
    /// パッチが必要なアドレス (forward reference)
    patches: Vec<Patch>,
    /// break 時にパッチするアドレスのスタック
    break_patches: Vec<Vec<usize>>,
}

#[derive(Debug)]
struct Patch {
    /// パッチ先のバイト位置
    offset: usize,
    /// パッチの種類
    kind: PatchKind,
}

#[derive(Debug)]
enum PatchKind {
    /// 関数呼び出し (CALL addr)
    Call(String),
    /// グローバル変数のアドレス (LD I, addr)
    GlobalAddr(String),
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            data: Vec::new(),
            fn_addrs: HashMap::new(),
            global_addrs: HashMap::new(),
            sprite_sizes: HashMap::new(),
            local_regs: HashMap::new(),
            next_reg: 0,
            bound_reg_count: 0,
            patches: Vec::new(),
            break_patches: Vec::new(),
        }
    }

    /// プログラム全体をコード生成し、バイトコードを返す
    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        // Pass 1: グローバル定数・スプライトをデータとして記録
        // (アドレスは後で確定)
        for top in &program.top_levels {
            if let TopLevel::LetDef {
                name, ty, value, ..
            } = top
            {
                let data_offset = self.data.len();
                match &value.kind {
                    ExprKind::ArrayLiteral(elems) => {
                        for elem in elems {
                            if let ExprKind::IntLiteral(v) = &elem.kind {
                                self.data.push(*v as u8);
                            }
                        }
                    }
                    ExprKind::IntLiteral(v) => {
                        self.data.push(*v as u8);
                    }
                    _ => {
                        self.data.push(0);
                    }
                }
                // アドレスは後で確定するのでオフセットとして保存
                self.global_addrs.insert(name.clone(), data_offset as u16);
                if let Type::Sprite(n) = ty {
                    self.sprite_sizes.insert(name.clone(), *n);
                }
            }
        }

        // main を呼ぶ JP 命令 (後でパッチ)
        let main_jp_offset = self.bytes.len();
        self.emit(0x00, 0x00); // placeholder for JP main

        // Pass 2: 関数を生成
        for top in &program.top_levels {
            if let TopLevel::FnDef {
                name, params, body, ..
            } = top
            {
                let addr = self.current_addr();
                self.fn_addrs.insert(name.clone(), addr);

                // ローカルスコープ
                self.local_regs.clear();
                self.next_reg = 0;
                self.bound_reg_count = 0;

                // 引数をレジスタに割り当て
                for param in params {
                    let reg = self.alloc_reg();
                    self.local_regs.insert(param.name.clone(), reg);
                }
                self.bound_reg_count = self.next_reg;

                // 本体を生成
                self.gen_expr(body);

                // RET (00EE)
                self.emit(0x00, 0xEE);
            }
        }

        // main への JP をパッチ
        if let Some(&main_addr) = self.fn_addrs.get("main") {
            let hi = (main_addr >> 8) as u8 | 0x10; // JP = 1NNN
            let lo = (main_addr & 0xFF) as u8;
            self.bytes[main_jp_offset] = hi;
            self.bytes[main_jp_offset + 1] = lo;
        }

        // グローバルデータのアドレスを確定
        let data_base = PROGRAM_START + self.bytes.len() as u16;
        let global_addrs = self.global_addrs.clone();
        for (name, offset) in &global_addrs {
            let addr = data_base + *offset;
            self.global_addrs.insert(name.clone(), addr);
        }

        // パッチを解決 (CALL + GlobalAddr)
        let patches = std::mem::take(&mut self.patches);
        for patch in &patches {
            match &patch.kind {
                PatchKind::Call(name) => {
                    if let Some(&addr) = self.fn_addrs.get(name) {
                        let hi = (addr >> 8) as u8 | 0x20; // CALL = 2NNN
                        let lo = (addr & 0xFF) as u8;
                        self.bytes[patch.offset] = hi;
                        self.bytes[patch.offset + 1] = lo;
                    }
                }
                PatchKind::GlobalAddr(name) => {
                    if let Some(&addr) = self.global_addrs.get(name) {
                        let hi = 0xA0 | ((addr >> 8) as u8 & 0x0F); // LD I = ANNN
                        let lo = (addr & 0xFF) as u8;
                        self.bytes[patch.offset] = hi;
                        self.bytes[patch.offset + 1] = lo;
                    }
                }
            }
        }

        // バイトコード + データを結合
        let mut result = self.bytes.clone();
        result.extend_from_slice(&self.data);
        result
    }

    fn current_addr(&self) -> u16 {
        PROGRAM_START + self.bytes.len() as u16
    }

    fn emit(&mut self, hi: u8, lo: u8) {
        self.bytes.push(hi);
        self.bytes.push(lo);
    }

    fn alloc_reg(&mut self) -> u8 {
        let reg = self.next_reg;
        self.next_reg += 1;
        reg
    }

    fn alloc_temp_reg(&mut self) -> u8 {
        self.alloc_reg()
    }

    fn get_reg(&self, name: &str) -> Option<u8> {
        self.local_regs.get(name).copied()
    }

    /// V0 がローカル変数にバインドされているかチェック
    fn v0_is_bound(&self) -> bool {
        self.local_regs.values().any(|&r| r == 0)
    }

    /// LD I, addr をパッチ予約付きで emit する
    fn emit_ld_i_global(&mut self, name: &str) {
        let offset = self.bytes.len();
        self.emit(0xA0, 0x00); // placeholder: LD I, 0x000
        self.patches.push(Patch {
            offset,
            kind: PatchKind::GlobalAddr(name.to_string()),
        });
    }

    /// グローバル変数を安全に読み込む (F065 のみ使用し、V0 を保護)
    /// addr のメモリから1バイトを target_reg に読み込む
    fn emit_global_read(&mut self, _addr: u16, target_reg: u8, name: &str) {
        if target_reg == 0 {
            self.emit_ld_i_global(name);
            self.emit(0xF0, 0x65); // LD V0, [I]
        } else if self.v0_is_bound() {
            self.emit(0x80 | target_reg, 0x00); // LD Vtarget, V0
            self.emit_ld_i_global(name);
            self.emit(0xF0, 0x65); // LD V0, [I]
            // XOR swap
            self.emit(0x80 | target_reg, 0x03);
            self.emit(0x80, (target_reg << 4) | 0x03);
            self.emit(0x80 | target_reg, 0x03);
        } else {
            self.emit_ld_i_global(name);
            self.emit(0xF0, 0x65); // LD V0, [I]
            self.emit(0x80 | target_reg, 0x00); // LD Vtarget, V0
        }
    }

    // ---- コード生成 ----

    fn gen_expr(&mut self, expr: &Expr) -> Option<u8> {
        match &expr.kind {
            ExprKind::IntLiteral(v) => {
                let reg = self.alloc_temp_reg();
                // LD Vx, byte (6XKK)
                self.emit(0x60 | reg, *v as u8);
                Some(reg)
            }
            ExprKind::BoolLiteral(b) => {
                let reg = self.alloc_temp_reg();
                let val = if *b { 1 } else { 0 };
                self.emit(0x60 | reg, val);
                Some(reg)
            }
            ExprKind::Ident(name) => {
                if let Some(reg) = self.get_reg(name) {
                    Some(reg)
                } else {
                    // グローバル変数: F065 (V0のみロード) を使って安全に読み込み
                    let reg = self.alloc_temp_reg();
                    if let Some(&addr) = self.global_addrs.get(name) {
                        self.emit_global_read(addr, reg, name);
                    }
                    Some(reg)
                }
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let lhs_reg = self.gen_expr(lhs)?;
                let rhs_reg = self.gen_expr(rhs)?;
                let result_reg = lhs_reg;
                match op {
                    BinOp::Add => {
                        // ADD Vx, Vy (8XY4)
                        self.emit(0x80 | lhs_reg, (rhs_reg << 4) | 0x04);
                    }
                    BinOp::Sub => {
                        // SUB Vx, Vy (8XY5)
                        self.emit(0x80 | lhs_reg, (rhs_reg << 4) | 0x05);
                    }
                    BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        // CHIP-8 にはこれらの命令がないため、
                        // ソフトウェア実装が必要だが簡略化のため未実装
                        // TODO: ソフトウェア乗除算
                    }
                    BinOp::Eq => {
                        // SE Vx, Vy (5XY0) → skip if equal
                        // 結果を result_reg に格納
                        let res = self.alloc_temp_reg();
                        self.emit(0x60 | res, 0); // LD res, 0
                        self.emit(0x50 | lhs_reg, rhs_reg << 4); // SE Vx, Vy
                        let skip_addr = self.current_addr() + 4; // skip next JP
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        ); // JP skip
                        self.emit(0x60 | res, 1); // LD res, 1
                        return Some(res);
                    }
                    BinOp::NotEq => {
                        let res = self.alloc_temp_reg();
                        self.emit(0x60 | res, 0);
                        // SNE Vx, Vy (9XY0) → skip if not equal
                        self.emit(0x90 | lhs_reg, rhs_reg << 4);
                        let skip_addr = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 1);
                        return Some(res);
                    }
                    BinOp::Lt => {
                        // Vx < Vy → SUB Vy, Vx; VF = NOT borrow
                        // VF=0 if Vy < Vx (borrow), VF=1 if Vy >= Vx
                        // 実際は Vx < Vy → SUBN Vx, Vy (8XY7); VF=1 if Vy > Vx
                        let res = self.alloc_temp_reg();
                        let tmp = self.alloc_temp_reg();
                        // tmp = lhs (copy)
                        self.emit(0x80 | tmp, lhs_reg << 4); // LD tmp, lhs
                        // SUBN tmp, rhs → tmp = rhs - tmp, VF = NOT borrow
                        self.emit(0x80 | tmp, (rhs_reg << 4) | 0x07);
                        // VF=1 if rhs > lhs (no borrow) → lhs < rhs
                        // SE VF, 1 → skip if lhs < rhs
                        self.emit(0x60 | res, 0); // LD res, 0
                        self.emit(0x3F, 0x01); // SE VF, 1
                        let skip_addr = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 1);
                        // ただし等しい場合は false にしたい
                        // 等しい場合: SUBN result = 0, VF=1
                        // → 追加チェック: lhs == rhs なら res = 0
                        self.emit(0x50 | lhs_reg, rhs_reg << 4); // SE lhs, rhs
                        let skip_addr2 = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr2 >> 8) as u8 & 0x0F),
                            (skip_addr2 & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 0); // equal → false
                        return Some(res);
                    }
                    BinOp::Gt => {
                        // lhs > rhs → rhs < lhs
                        let res = self.alloc_temp_reg();
                        let tmp = self.alloc_temp_reg();
                        self.emit(0x80 | tmp, rhs_reg << 4); // LD tmp, rhs
                        self.emit(0x80 | tmp, (lhs_reg << 4) | 0x07); // SUBN tmp, lhs
                        self.emit(0x60 | res, 0);
                        self.emit(0x3F, 0x01); // SE VF, 1
                        let skip_addr = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 1);
                        self.emit(0x50 | rhs_reg, lhs_reg << 4); // SE rhs, lhs
                        let skip_addr2 = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr2 >> 8) as u8 & 0x0F),
                            (skip_addr2 & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 0);
                        return Some(res);
                    }
                    BinOp::LtEq => {
                        // lhs <= rhs → !(lhs > rhs)
                        // Gt の反転
                        let res = self.alloc_temp_reg();
                        let tmp = self.alloc_temp_reg();
                        self.emit(0x80 | tmp, rhs_reg << 4);
                        self.emit(0x80 | tmp, (lhs_reg << 4) | 0x07);
                        self.emit(0x60 | res, 1); // assume true
                        self.emit(0x3F, 0x01); // SE VF, 1
                        let skip_addr = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 0); // lhs > rhs → false
                        // 等しい場合は true (already 1)
                        return Some(res);
                    }
                    BinOp::GtEq => {
                        // lhs >= rhs → !(lhs < rhs)
                        let res = self.alloc_temp_reg();
                        let tmp = self.alloc_temp_reg();
                        self.emit(0x80 | tmp, lhs_reg << 4);
                        self.emit(0x80 | tmp, (rhs_reg << 4) | 0x07);
                        self.emit(0x60 | res, 1); // assume true
                        self.emit(0x3F, 0x01); // SE VF, 1
                        let skip_addr = self.current_addr() + 4;
                        self.emit(
                            0x10 | ((skip_addr >> 8) as u8 & 0x0F),
                            (skip_addr & 0xFF) as u8,
                        );
                        self.emit(0x60 | res, 0);
                        return Some(res);
                    }
                    BinOp::And => {
                        // AND Vx, Vy (8XY2)
                        self.emit(0x80 | lhs_reg, (rhs_reg << 4) | 0x02);
                    }
                    BinOp::Or => {
                        // OR Vx, Vy (8XY1)
                        self.emit(0x80 | lhs_reg, (rhs_reg << 4) | 0x01);
                    }
                }
                Some(result_reg)
            }
            ExprKind::UnaryOp { op, expr: inner } => {
                let reg = self.gen_expr(inner)?;
                match op {
                    UnaryOp::Neg => {
                        // 0 - Vx
                        let zero = self.alloc_temp_reg();
                        self.emit(0x60 | zero, 0x00); // LD zero, 0
                        self.emit(0x80 | zero, (reg << 4) | 0x05); // SUB zero, reg
                        Some(zero)
                    }
                    UnaryOp::Not => {
                        // XOR with 1
                        let one = self.alloc_temp_reg();
                        self.emit(0x60 | one, 0x01);
                        self.emit(0x80 | reg, (one << 4) | 0x03); // XOR reg, one
                        Some(reg)
                    }
                }
            }
            ExprKind::Call { name, args } => {
                // 引数をレジスタに配置
                // draw() の場合、args[0] はスプライト名なので評価をスキップ
                let is_draw = name == "draw";
                let mut arg_regs = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    if is_draw && i == 0 {
                        // draw のスプライト引数は名前参照のみ、評価不要
                        arg_regs.push(0); // placeholder
                        continue;
                    }
                    if let Some(reg) = self.gen_expr(arg) {
                        arg_regs.push(reg);
                    }
                }

                // 組み込み関数のコード生成
                match name.as_str() {
                    "clear" => {
                        self.emit(0x00, 0xE0); // CLS
                        Some(0)
                    }
                    "draw" => {
                        // draw(sprite_name, x, y)
                        // args[0] はスプライト変数名
                        if args.len() == 3 {
                            // スプライトのアドレスを I にセット (パッチ予約)
                            if let ExprKind::Ident(sprite_name) = &args[0].kind {
                                self.emit_ld_i_global(sprite_name);
                                let n =
                                    self.sprite_sizes.get(sprite_name).copied().unwrap_or(1) as u8;
                                let x_reg = arg_regs[1];
                                let y_reg = arg_regs[2];
                                // DXYN
                                self.emit(0xD0 | x_reg, (y_reg << 4) | n);
                            }
                        }
                        // VF にコリジョン
                        Some(0x0F)
                    }
                    "wait_key" => {
                        let reg = if arg_regs.is_empty() {
                            self.alloc_temp_reg()
                        } else {
                            arg_regs[0]
                        };
                        // FX0A
                        self.emit(0xF0 | reg, 0x0A);
                        Some(reg)
                    }
                    "is_key_pressed" => {
                        let key_reg = arg_regs[0];
                        let res = self.alloc_temp_reg();
                        self.emit(0x60 | res, 1); // LD res, 1 (assume pressed)
                        // EX9E: skip next if key IS pressed → keep res=1
                        self.emit(0xE0 | key_reg, 0x9E);
                        self.emit(0x60 | res, 0); // LD res, 0 (not pressed)
                        Some(res)
                    }
                    "delay" => {
                        let reg = self.alloc_temp_reg();
                        // FX07
                        self.emit(0xF0 | reg, 0x07);
                        Some(reg)
                    }
                    "set_delay" => {
                        let reg = arg_regs[0];
                        // FX15
                        self.emit(0xF0 | reg, 0x15);
                        Some(0)
                    }
                    "set_sound" => {
                        let reg = arg_regs[0];
                        // FX18
                        self.emit(0xF0 | reg, 0x18);
                        Some(0)
                    }
                    "random" => {
                        let mask_reg = arg_regs[0];
                        let res = self.alloc_temp_reg();
                        // CXKK - random AND kk
                        // mask は定数の方が望ましいが、レジスタの値を使う
                        // → 簡易実装: CXFF して AND mask_reg
                        self.emit(0xC0 | res, 0xFF);
                        self.emit(0x80 | res, (mask_reg << 4) | 0x02); // AND
                        Some(res)
                    }
                    "bcd" => {
                        let reg = arg_regs[0];
                        // FX33
                        self.emit(0xF0 | reg, 0x33);
                        Some(0)
                    }
                    "draw_digit" => {
                        if arg_regs.len() >= 3 {
                            let val_reg = arg_regs[0];
                            let x_reg = arg_regs[1];
                            let y_reg = arg_regs[2];
                            // FX29: LD F, Vx (font sprite address)
                            self.emit(0xF0 | val_reg, 0x29);
                            // DXY5: draw 5-byte sprite
                            self.emit(0xD0 | x_reg, (y_reg << 4) | 5);
                        }
                        Some(0)
                    }
                    _ => {
                        // ユーザー定義関数
                        // 引数をV0, V1, ... にコピー
                        for (i, &arg_reg) in arg_regs.iter().enumerate() {
                            if arg_reg != i as u8 {
                                self.emit(0x80 | (i as u8), arg_reg << 4);
                                // LD Vi, arg_reg
                            }
                        }
                        // CALL (2NNN) - パッチ予約
                        let offset = self.bytes.len();
                        self.emit(0x00, 0x00); // placeholder
                        self.patches.push(Patch {
                            offset,
                            kind: PatchKind::Call(name.clone()),
                        });
                        Some(0)
                    }
                }
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_reg = self.gen_expr(cond)?;
                // SE cond, 1 → skip if true
                // if cond is false (0), skip then block
                self.emit(0x40 | cond_reg, 0x00); // SNE Vx, 0 → skip JP when cond is true
                let jp_else_offset = self.bytes.len();
                self.emit(0x00, 0x00); // JP else (placeholder)

                let then_reg = self.gen_expr(then_block);

                if let Some(else_block) = else_block {
                    let jp_end_offset = self.bytes.len();
                    self.emit(0x00, 0x00); // JP end (placeholder)

                    // else ブロック
                    let else_addr = self.current_addr();
                    self.bytes[jp_else_offset] = 0x10 | ((else_addr >> 8) as u8 & 0x0F);
                    self.bytes[jp_else_offset + 1] = (else_addr & 0xFF) as u8;

                    let else_reg = self.gen_expr(else_block);

                    // then の結果を else の結果レジスタにコピー
                    if let (Some(tr), Some(er)) = (then_reg, else_reg)
                        && tr != er
                    {
                        // 戻ってから結果を統一する必要があるが、
                        // 簡略化のため then_reg をそのまま使う
                    }

                    let end_addr = self.current_addr();
                    self.bytes[jp_end_offset] = 0x10 | ((end_addr >> 8) as u8 & 0x0F);
                    self.bytes[jp_end_offset + 1] = (end_addr & 0xFF) as u8;

                    else_reg.or(then_reg)
                } else {
                    // else なし
                    let end_addr = self.current_addr();
                    self.bytes[jp_else_offset] = 0x10 | ((end_addr >> 8) as u8 & 0x0F);
                    self.bytes[jp_else_offset + 1] = (end_addr & 0xFF) as u8;
                    then_reg
                }
            }
            ExprKind::Loop { body } => {
                let loop_addr = self.current_addr();
                self.break_patches.push(Vec::new());

                self.gen_expr(body);

                // JP loop_addr (1NNN)
                self.emit(
                    0x10 | ((loop_addr >> 8) as u8 & 0x0F),
                    (loop_addr & 0xFF) as u8,
                );

                // break パッチを解決
                let end_addr = self.current_addr();
                if let Some(break_offsets) = self.break_patches.pop() {
                    for offset in break_offsets {
                        self.bytes[offset] = 0x10 | ((end_addr >> 8) as u8 & 0x0F);
                        self.bytes[offset + 1] = (end_addr & 0xFF) as u8;
                    }
                }

                None
            }
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    self.gen_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.gen_expr(tail)
                } else {
                    None
                }
            }
            ExprKind::ArrayLiteral(_) => {
                // 配列リテラルはグローバルのデータセクションで処理済み
                None
            }
            ExprKind::Index { array, index } => {
                // 配列アクセス: I + index でメモリから読み込み
                if let ExprKind::Ident(name) = &array.kind
                    && self.global_addrs.contains_key(name)
                {
                    let idx_reg = self.gen_expr(index)?;
                    let result_reg = self.alloc_temp_reg();
                    // I = base_addr (パッチ予約)
                    self.emit_ld_i_global(name);
                    // I += idx_reg (FX1E: ADD I, Vx)
                    self.emit(0xF0 | idx_reg, 0x1E);
                    // 安全に V0 経由で読み込み (F065 のみ使用)
                    if result_reg == 0 {
                        self.emit(0xF0, 0x65); // LD V0, [I]
                    } else if self.v0_is_bound() {
                        // V0 退避 → 読み込み → XOR swap
                        self.emit(0x80 | result_reg, 0x00); // LD Vresult, V0
                        self.emit(0xF0, 0x65); // LD V0, [I]
                        self.emit(0x80 | result_reg, 0x03); // XOR
                        self.emit(0x80, (result_reg << 4) | 0x03);
                        self.emit(0x80 | result_reg, 0x03);
                    } else {
                        self.emit(0xF0, 0x65); // LD V0, [I]
                        self.emit(0x80 | result_reg, 0x00); // LD Vresult, V0
                    }
                    return Some(result_reg);
                }
                None
            }
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                if let Some(val_reg) = self.gen_expr(value) {
                    // temp レジスタをリセット (bound の直後から再開)
                    self.next_reg = self.bound_reg_count;
                    let reg = self.alloc_reg();
                    if val_reg != reg {
                        // LD reg, val_reg (8XY0)
                        self.emit(0x80 | reg, val_reg << 4);
                    }
                    self.local_regs.insert(name.clone(), reg);
                    // 新しいバインド変数を反映
                    self.bound_reg_count = self.next_reg;
                    return; // early return: next_reg は bound_reg_count に設定済み
                }
            }
            StmtKind::Assign { name, value } => {
                if let Some(val_reg) = self.gen_expr(value)
                    && let Some(&target_reg) = self.local_regs.get(name)
                    && val_reg != target_reg
                {
                    self.emit(0x80 | target_reg, val_reg << 4);
                }
            }
            StmtKind::Expr(expr) => {
                self.gen_expr(expr);
            }
            StmtKind::Return(expr) => {
                if let Some(e) = expr
                    && let Some(reg) = self.gen_expr(e)
                    && reg != 0
                {
                    // 戻り値を V0 にコピー
                    self.emit(0x80, reg << 4);
                }
                self.emit(0x00, 0xEE); // RET
            }
            StmtKind::Break => {
                // JP end (placeholder)
                let offset = self.bytes.len();
                self.emit(0x00, 0x00);
                if let Some(patches) = self.break_patches.last_mut() {
                    patches.push(offset);
                }
            }
        }

        // 文の終了後、temp レジスタをリセット
        // bound_reg_count より上のレジスタは再利用可能
        self.next_reg = self.bound_reg_count;
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}
