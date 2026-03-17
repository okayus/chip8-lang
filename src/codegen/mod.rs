use std::collections::HashMap;

use crate::chip8::{Addr, ByteOffset, Opcode, Register, SpriteHeight, UserRegister};
use crate::parser::ast::*;

/// CHIP-8 命令のバイト数
const INSTRUCTION_SIZE: u16 = 2;

/// コード生成した値の所在
#[derive(Clone, Copy)]
enum ValueLocation {
    /// 値はレジスタに格納されている
    InRegister(Register),
    /// 値を生成しない式 (loop, 配列リテラルなど)
    Void,
}

impl ValueLocation {
    fn register(self) -> Option<Register> {
        match self {
            ValueLocation::InRegister(r) => Some(r),
            ValueLocation::Void => None,
        }
    }
}

/// 前方参照: アドレスが未確定の命令を後からパッチするための記録
#[derive(Debug)]
struct ForwardRef {
    /// パッチ先のバイト位置
    offset: ByteOffset,
    /// パッチの種類
    kind: ForwardRefKind,
}

/// 前方参照の種類
#[derive(Debug)]
enum ForwardRefKind {
    /// 関数呼び出し (CALL addr)
    Call(String),
    /// グローバル変数のアドレス (LD I, addr)
    GlobalAddr(String),
}

/// コード生成器
///
/// AST を走査して CHIP-8 バイトコードを生成する。
/// Pass 1 でグローバルデータを収集し、Pass 2 で関数本体のコードを生成する。
pub struct CodeGen {
    /// 生成されたバイトコード
    bytes: Vec<u8>,
    /// データセクション (スプライトなど)
    data: Vec<u8>,
    /// 関数名 → 確定アドレス
    fn_addrs: HashMap<String, Addr>,
    /// グローバル変数名 → データセクション内のバイトオフセット (Pass 1 で記録)
    data_offsets: HashMap<String, u16>,
    /// グローバル変数名 → 最終解決済みアドレス (generate() 末尾で確定)
    resolved_addrs: HashMap<String, Addr>,
    /// グローバル変数のスプライトサイズ (バイト数)
    sprite_sizes: HashMap<String, usize>,
    /// enum variant → u8 値
    enum_variant_values: HashMap<(String, String), u8>,
    /// レジスタ退避用の次のスロットアドレス
    next_save_slot: u16,
    /// ローカル変数名 → 割り当て済みレジスタ
    local_bindings: HashMap<String, UserRegister>,
    /// 次に割り当て可能なレジスタ番号
    next_free_reg: u8,
    /// ローカル変数にバインド済みのレジスタ数 (一時レジスタのリセット基準)
    local_var_count: u8,
    /// アドレス未確定の前方参照リスト
    forward_refs: Vec<ForwardRef>,
    /// ループごとの break 先パッチオフセットのスタック
    loop_break_offsets: Vec<Vec<ByteOffset>>,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            data: Vec::new(),
            fn_addrs: HashMap::new(),
            data_offsets: HashMap::new(),
            resolved_addrs: HashMap::new(),
            sprite_sizes: HashMap::new(),
            enum_variant_values: HashMap::new(),
            next_save_slot: 0x0A0,
            local_bindings: HashMap::new(),
            next_free_reg: 0,
            local_var_count: 0,
            forward_refs: Vec::new(),
            loop_break_offsets: Vec::new(),
        }
    }

    /// プログラム全体をコード生成し、バイトコードを返す
    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        // Pass 0: enum 定義を登録
        for top in &program.top_levels {
            if let TopLevel::EnumDef { name, variants, .. } = top {
                for (i, variant) in variants.iter().enumerate() {
                    self.enum_variant_values
                        .insert((name.clone(), variant.clone()), i as u8);
                }
            }
        }

        // Pass 1: グローバル定数・スプライトをデータとして記録
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
                self.data_offsets.insert(name.clone(), data_offset as u16);
                if let Type::Sprite(n) = ty {
                    self.sprite_sizes.insert(name.clone(), *n);
                }
            }
        }

        // main を呼ぶ JP 命令 (後でパッチ)
        let main_jp_offset = self.emit_placeholder();

        // Pass 2: 関数を生成
        for top in &program.top_levels {
            if let TopLevel::FnDef {
                name, params, body, ..
            } = top
            {
                let addr = self.current_addr();
                self.fn_addrs.insert(name.clone(), addr);

                self.local_bindings.clear();
                self.next_free_reg = 0;
                self.local_var_count = 0;

                for param in params {
                    let reg = self.alloc_register();
                    self.local_bindings.insert(param.name.clone(), reg);
                }
                self.local_var_count = self.next_free_reg;

                self.codegen_expr(body);
                self.emit_op(Opcode::Ret);
            }
        }

        // main への JP をパッチ
        if let Some(&main_addr) = self.fn_addrs.get("main") {
            self.patch_at(main_jp_offset, Opcode::Jp(main_addr));
        }

        // グローバルデータのアドレスを確定: data_offsets → resolved_addrs
        let data_base = Addr::PROGRAM_START.raw() + self.bytes.len() as u16;
        for (name, offset) in &self.data_offsets {
            self.resolved_addrs
                .insert(name.clone(), Addr::new(data_base + *offset));
        }

        // 前方参照を解決
        let forward_refs = std::mem::take(&mut self.forward_refs);
        for fref in &forward_refs {
            match &fref.kind {
                ForwardRefKind::Call(name) => {
                    if let Some(&addr) = self.fn_addrs.get(name) {
                        self.patch_at(fref.offset, Opcode::Call(addr));
                    }
                }
                ForwardRefKind::GlobalAddr(name) => {
                    if let Some(&addr) = self.resolved_addrs.get(name) {
                        self.patch_at(fref.offset, Opcode::LdI(addr));
                    }
                }
            }
        }

        // バイトコード + データを結合
        let mut result = self.bytes.clone();
        result.extend_from_slice(&self.data);
        result
    }

    fn current_addr(&self) -> Addr {
        Addr::new(Addr::PROGRAM_START.raw() + self.bytes.len() as u16)
    }

    /// 次の 1 命令を飛び越えた先のアドレス
    /// (SE/SNE 条件スキップ + JP パターンで使用)
    fn skip_next_addr(&self) -> Addr {
        Addr::new(self.current_addr().raw() + 2 * INSTRUCTION_SIZE)
    }

    fn emit_op(&mut self, op: Opcode) {
        let [hi, lo] = op.encode();
        self.bytes.push(hi);
        self.bytes.push(lo);
    }

    fn emit_placeholder(&mut self) -> ByteOffset {
        let offset = ByteOffset(self.bytes.len());
        self.bytes.push(0x00);
        self.bytes.push(0x00);
        offset
    }

    fn patch_at(&mut self, offset: ByteOffset, op: Opcode) {
        let [hi, lo] = op.encode();
        self.bytes[offset.0] = hi;
        self.bytes[offset.0 + 1] = lo;
    }

    fn alloc_register(&mut self) -> UserRegister {
        assert!(
            self.next_free_reg <= 14,
            "register allocation overflow: attempted to allocate beyond V14 (VE)"
        );
        let reg = UserRegister::new(self.next_free_reg);
        self.next_free_reg += 1;
        reg
    }

    fn alloc_temp_register(&mut self) -> UserRegister {
        self.alloc_register()
    }

    fn lookup_register(&self, name: &str) -> Option<UserRegister> {
        self.local_bindings.get(name).copied()
    }

    fn v0_is_bound(&self) -> bool {
        self.local_bindings.values().any(|r| r.index() == 0)
    }

    fn emit_ld_i_global(&mut self, name: &str) {
        let offset = self.emit_placeholder();
        self.forward_refs.push(ForwardRef {
            offset,
            kind: ForwardRefKind::GlobalAddr(name.to_string()),
        });
    }

    fn emit_global_read(&mut self, target_reg: Register, name: &str) {
        if target_reg == Register::V0 {
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdVxI(Register::V0));
        } else if self.v0_is_bound() {
            self.emit_op(Opcode::LdReg(target_reg, Register::V0));
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdVxI(Register::V0));
            self.emit_op(Opcode::Xor(target_reg, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, target_reg));
            self.emit_op(Opcode::Xor(target_reg, Register::V0));
        } else {
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdVxI(Register::V0));
            self.emit_op(Opcode::LdReg(target_reg, Register::V0));
        }
    }

    fn pattern_value(&self, pattern: &Expr) -> u8 {
        match &pattern.kind {
            ExprKind::IntLiteral(v) => *v as u8,
            ExprKind::EnumVariant { enum_name, variant } => self
                .enum_variant_values
                .get(&(enum_name.clone(), variant.clone()))
                .copied()
                .unwrap_or(0),
            _ => 0,
        }
    }

    // ---- コード生成 ----

    fn codegen_expr(&mut self, expr: &Expr) -> ValueLocation {
        match &expr.kind {
            ExprKind::IntLiteral(v) => {
                let reg = self.alloc_temp_register();
                self.emit_op(Opcode::LdImm(reg.into(), *v as u8));
                ValueLocation::InRegister(reg.into())
            }
            ExprKind::BoolLiteral(b) => {
                let reg = self.alloc_temp_register();
                let val = if *b { 1 } else { 0 };
                self.emit_op(Opcode::LdImm(reg.into(), val));
                ValueLocation::InRegister(reg.into())
            }
            ExprKind::Ident(name) => {
                if let Some(reg) = self.lookup_register(name) {
                    ValueLocation::InRegister(reg.into())
                } else {
                    let reg = self.alloc_temp_register();
                    if self.data_offsets.contains_key(name) {
                        self.emit_global_read(reg.into(), name);
                    }
                    ValueLocation::InRegister(reg.into())
                }
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let Some(lhs_reg) = self.codegen_expr(lhs).register() else {
                    return ValueLocation::Void;
                };
                let Some(rhs_reg) = self.codegen_expr(rhs).register() else {
                    return ValueLocation::Void;
                };
                let result_reg = lhs_reg;
                match op {
                    BinOp::Add => {
                        self.emit_op(Opcode::Add(lhs_reg, rhs_reg));
                    }
                    BinOp::Sub => {
                        self.emit_op(Opcode::Sub(lhs_reg, rhs_reg));
                    }
                    BinOp::Mul | BinOp::Div | BinOp::Mod => {}
                    BinOp::Eq => {
                        let res = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SeReg(lhs_reg, rhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::NotEq => {
                        let res = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SneReg(lhs_reg, rhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::Lt => {
                        let res = self.alloc_temp_register();
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdReg(tmp.into(), lhs_reg));
                        self.emit_op(Opcode::Subn(tmp.into(), rhs_reg));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SeImm(Register::VF, 0x01));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        self.emit_op(Opcode::SeReg(lhs_reg, rhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::Gt => {
                        let res = self.alloc_temp_register();
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdReg(tmp.into(), rhs_reg));
                        self.emit_op(Opcode::Subn(tmp.into(), lhs_reg));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SeImm(Register::VF, 0x01));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        self.emit_op(Opcode::SeReg(rhs_reg, lhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::LtEq => {
                        let res = self.alloc_temp_register();
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdReg(tmp.into(), rhs_reg));
                        self.emit_op(Opcode::Subn(tmp.into(), lhs_reg));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        self.emit_op(Opcode::SeImm(Register::VF, 0x01));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::GtEq => {
                        let res = self.alloc_temp_register();
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdReg(tmp.into(), lhs_reg));
                        self.emit_op(Opcode::Subn(tmp.into(), rhs_reg));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                        self.emit_op(Opcode::SeImm(Register::VF, 0x01));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        return ValueLocation::InRegister(res.into());
                    }
                    BinOp::And => {
                        self.emit_op(Opcode::And(lhs_reg, rhs_reg));
                    }
                    BinOp::Or => {
                        self.emit_op(Opcode::Or(lhs_reg, rhs_reg));
                    }
                }
                ValueLocation::InRegister(result_reg)
            }
            ExprKind::UnaryOp { op, expr: inner } => {
                let Some(reg) = self.codegen_expr(inner).register() else {
                    return ValueLocation::Void;
                };
                match op {
                    UnaryOp::Neg => {
                        let zero = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(zero.into(), 0x00));
                        self.emit_op(Opcode::Sub(zero.into(), reg));
                        ValueLocation::InRegister(zero.into())
                    }
                    UnaryOp::Not => {
                        let one = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(one.into(), 0x01));
                        self.emit_op(Opcode::Xor(reg, one.into()));
                        ValueLocation::InRegister(reg)
                    }
                }
            }
            ExprKind::BuiltinCall { builtin, args } => self.codegen_builtin_call(*builtin, args),
            ExprKind::Call { name, args } => {
                // ユーザー定義関数: 引数を評価して V0, V1, ... にコピー
                let mut arg_regs = Vec::new();
                for arg in args {
                    if let Some(reg) = self.codegen_expr(arg).register() {
                        arg_regs.push(reg);
                    }
                }
                let num_to_save = self.local_var_count;

                // caller-save: ローカル変数をメモリに退避
                let save_addr = self.next_save_slot;
                if num_to_save > 0 {
                    self.emit_op(Opcode::LdI(Addr::new(save_addr)));
                    let last_reg = UserRegister::new(num_to_save - 1);
                    self.emit_op(Opcode::LdIVx(last_reg.into()));
                    self.next_save_slot += num_to_save as u16;
                }

                // 引数を V0, V1, ... にコピー
                for (i, &arg_reg) in arg_regs.iter().enumerate() {
                    let target: Register = UserRegister::new(i as u8).into();
                    if arg_reg != target {
                        self.emit_op(Opcode::LdReg(target, arg_reg));
                    }
                }

                // CALL
                let offset = self.emit_placeholder();
                self.forward_refs.push(ForwardRef {
                    offset,
                    kind: ForwardRefKind::Call(name.clone()),
                });

                // 戻り値を退避してからレジスタを復帰
                let result_reg = if num_to_save > 0 {
                    let temp = UserRegister::new(num_to_save);
                    self.emit_op(Opcode::LdReg(temp.into(), Register::V0));
                    self.emit_op(Opcode::LdI(Addr::new(save_addr)));
                    let last_reg = UserRegister::new(num_to_save - 1);
                    self.emit_op(Opcode::LdVxI(last_reg.into()));
                    temp.into()
                } else {
                    Register::V0
                };

                ValueLocation::InRegister(result_reg)
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let Some(cond_reg) = self.codegen_expr(cond).register() else {
                    return ValueLocation::Void;
                };
                self.emit_op(Opcode::SneImm(cond_reg, 0x00));
                let jp_else_offset = self.emit_placeholder();

                let then_loc = self.codegen_expr(then_block);

                if let Some(else_block) = else_block {
                    let jp_end_offset = self.emit_placeholder();

                    let else_addr = self.current_addr();
                    self.patch_at(jp_else_offset, Opcode::Jp(else_addr));

                    let else_loc = self.codegen_expr(else_block);

                    if let (Some(_tr), Some(_er)) = (then_loc.register(), else_loc.register()) {}

                    let end_addr = self.current_addr();
                    self.patch_at(jp_end_offset, Opcode::Jp(end_addr));

                    match (then_loc, else_loc) {
                        (_, ValueLocation::InRegister(r)) => ValueLocation::InRegister(r),
                        (ValueLocation::InRegister(r), _) => ValueLocation::InRegister(r),
                        _ => ValueLocation::Void,
                    }
                } else {
                    let end_addr = self.current_addr();
                    self.patch_at(jp_else_offset, Opcode::Jp(end_addr));
                    then_loc
                }
            }
            ExprKind::Loop { body } => {
                let loop_addr = self.current_addr();
                self.loop_break_offsets.push(Vec::new());

                self.codegen_expr(body);

                self.emit_op(Opcode::Jp(loop_addr));

                let end_addr = self.current_addr();
                if let Some(break_offsets) = self.loop_break_offsets.pop() {
                    for offset in break_offsets {
                        self.patch_at(offset, Opcode::Jp(end_addr));
                    }
                }

                ValueLocation::Void
            }
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    self.codegen_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.codegen_expr(tail)
                } else {
                    ValueLocation::Void
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                let Some(scr_reg) = self.codegen_expr(scrutinee).register() else {
                    return ValueLocation::Void;
                };
                if arms.is_empty() {
                    return ValueLocation::Void;
                }
                if arms.len() == 1 {
                    return self.codegen_expr(&arms[0].body);
                }
                let mut end_offsets = Vec::new();
                let last_idx = arms.len() - 1;
                for (i, arm) in arms.iter().enumerate() {
                    if i == last_idx {
                        // 最終アーム: デフォルト (条件なし)
                        let loc = self.codegen_expr(&arm.body);
                        let end_addr = self.current_addr();
                        for off in &end_offsets {
                            self.patch_at(*off, Opcode::Jp(end_addr));
                        }
                        return loc;
                    }
                    let pattern_val = self.pattern_value(&arm.pattern);
                    self.emit_op(Opcode::SneImm(scr_reg, pattern_val));
                    let jp_next_arm = self.emit_placeholder();
                    self.codegen_expr(&arm.body);
                    let jp_end = self.emit_placeholder();
                    end_offsets.push(jp_end);
                    let next_addr = self.current_addr();
                    self.patch_at(jp_next_arm, Opcode::Jp(next_addr));
                }
                ValueLocation::Void
            }
            ExprKind::EnumVariant { enum_name, variant } => {
                let val = self
                    .enum_variant_values
                    .get(&(enum_name.clone(), variant.clone()))
                    .copied()
                    .unwrap_or(0);
                let reg = self.alloc_temp_register();
                self.emit_op(Opcode::LdImm(reg.into(), val));
                ValueLocation::InRegister(reg.into())
            }
            ExprKind::ArrayLiteral(_) => ValueLocation::Void,
            ExprKind::Index { array, index } => {
                if let ExprKind::Ident(name) = &array.kind
                    && self.data_offsets.contains_key(name)
                {
                    let Some(idx_reg) = self.codegen_expr(index).register() else {
                        return ValueLocation::Void;
                    };
                    let result_reg = self.alloc_temp_register();
                    self.emit_ld_i_global(name);
                    self.emit_op(Opcode::AddI(idx_reg));
                    if Register::from(result_reg) == Register::V0 {
                        self.emit_op(Opcode::LdVxI(Register::V0));
                    } else if self.v0_is_bound() {
                        self.emit_op(Opcode::LdReg(result_reg.into(), Register::V0));
                        self.emit_op(Opcode::LdVxI(Register::V0));
                        self.emit_op(Opcode::Xor(result_reg.into(), Register::V0));
                        self.emit_op(Opcode::Xor(Register::V0, result_reg.into()));
                        self.emit_op(Opcode::Xor(result_reg.into(), Register::V0));
                    } else {
                        self.emit_op(Opcode::LdVxI(Register::V0));
                        self.emit_op(Opcode::LdReg(result_reg.into(), Register::V0));
                    }
                    return ValueLocation::InRegister(result_reg.into());
                }
                ValueLocation::Void
            }
        }
    }

    /// 組み込み関数のコード生成 (exhaustive match で全バリアントをカバー)
    fn codegen_builtin_call(&mut self, builtin: BuiltinFunction, args: &[Expr]) -> ValueLocation {
        // draw は args[0] をスプライト名参照として扱い、評価しない
        let is_draw = builtin == BuiltinFunction::Draw;
        let mut arg_regs = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            if is_draw && i == 0 {
                arg_regs.push(Register::V0); // placeholder
                continue;
            }
            if let Some(reg) = self.codegen_expr(arg).register() {
                arg_regs.push(reg);
            }
        }

        match builtin {
            BuiltinFunction::Clear => {
                self.emit_op(Opcode::Cls);
                ValueLocation::InRegister(Register::V0)
            }
            BuiltinFunction::Draw => {
                if args.len() == 3
                    && let ExprKind::Ident(sprite_name) = &args[0].kind
                {
                    self.emit_ld_i_global(sprite_name);
                    let n = self.sprite_sizes.get(sprite_name).copied().unwrap_or(1) as u8;
                    let x_reg = arg_regs[1];
                    let y_reg = arg_regs[2];
                    self.emit_op(Opcode::Drw(x_reg, y_reg, SpriteHeight::new(n)));
                }
                ValueLocation::InRegister(Register::VF)
            }
            BuiltinFunction::WaitKey => {
                let reg: Register = if arg_regs.is_empty() {
                    self.alloc_temp_register().into()
                } else {
                    arg_regs[0]
                };
                self.emit_op(Opcode::LdVxK(reg));
                ValueLocation::InRegister(reg)
            }
            BuiltinFunction::IsKeyPressed => {
                let key_reg = arg_regs[0];
                let res = self.alloc_temp_register();
                self.emit_op(Opcode::LdImm(res.into(), 1));
                self.emit_op(Opcode::Skp(key_reg));
                self.emit_op(Opcode::LdImm(res.into(), 0));
                ValueLocation::InRegister(res.into())
            }
            BuiltinFunction::Delay => {
                let reg = self.alloc_temp_register();
                self.emit_op(Opcode::LdVxDt(reg.into()));
                ValueLocation::InRegister(reg.into())
            }
            BuiltinFunction::SetDelay => {
                let reg = arg_regs[0];
                self.emit_op(Opcode::LdDtVx(reg));
                ValueLocation::InRegister(Register::V0)
            }
            BuiltinFunction::SetSound => {
                let reg = arg_regs[0];
                self.emit_op(Opcode::LdStVx(reg));
                ValueLocation::InRegister(Register::V0)
            }
            BuiltinFunction::Random => {
                let mask_reg = arg_regs[0];
                let res = self.alloc_temp_register();
                self.emit_op(Opcode::Rnd(res.into(), 0xFF));
                self.emit_op(Opcode::And(res.into(), mask_reg));
                ValueLocation::InRegister(res.into())
            }
            BuiltinFunction::Bcd => {
                let reg = arg_regs[0];
                self.emit_op(Opcode::LdBVx(reg));
                ValueLocation::InRegister(Register::V0)
            }
            BuiltinFunction::DrawDigit => {
                if arg_regs.len() >= 3 {
                    let val_reg = arg_regs[0];
                    let x_reg = arg_regs[1];
                    let y_reg = arg_regs[2];
                    self.emit_op(Opcode::LdFVx(val_reg));
                    self.emit_op(Opcode::Drw(x_reg, y_reg, SpriteHeight::new(5)));
                }
                ValueLocation::InRegister(Register::V0)
            }
        }
    }

    fn codegen_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                if let Some(val_reg) = self.codegen_expr(value).register() {
                    self.next_free_reg = self.local_var_count;
                    let reg = self.alloc_register();
                    if val_reg != Register::from(reg) {
                        self.emit_op(Opcode::LdReg(reg.into(), val_reg));
                    }
                    self.local_bindings.insert(name.clone(), reg);
                    self.local_var_count = self.next_free_reg;
                    return;
                }
            }
            StmtKind::Assign { name, value } => {
                if let Some(val_reg) = self.codegen_expr(value).register()
                    && let Some(&target_reg) = self.local_bindings.get(name)
                    && val_reg != Register::from(target_reg)
                {
                    self.emit_op(Opcode::LdReg(target_reg.into(), val_reg));
                }
            }
            StmtKind::Expr(expr) => {
                self.codegen_expr(expr);
            }
            StmtKind::Return(expr) => {
                if let Some(e) = expr
                    && let Some(reg) = self.codegen_expr(e).register()
                    && reg != Register::V0
                {
                    self.emit_op(Opcode::LdReg(Register::V0, reg));
                }
                self.emit_op(Opcode::Ret);
            }
            StmtKind::Break => {
                let offset = self.emit_placeholder();
                if let Some(offsets) = self.loop_break_offsets.last_mut() {
                    offsets.push(offset);
                }
            }
        }

        self.next_free_reg = self.local_var_count;
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}
