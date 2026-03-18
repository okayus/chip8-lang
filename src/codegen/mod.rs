use std::collections::{HashMap, HashSet};

use crate::chip8::{Addr, ByteOffset, Opcode, Register, SpriteHeight, UserRegister};
use crate::parser::ast::*;

/// CHIP-8 命令のバイト数
const INSTRUCTION_SIZE: u16 = 2;

/// コード生成した値の所在
#[derive(Clone)]
enum ValueLocation {
    /// 値はレジスタに格納されている
    InRegister(Register),
    /// struct の値: メモリに格納 (アドレスと struct 名)
    InMemory { addr: u16, struct_name: String },
    /// struct の値: 連続レジスタに保持
    InRegisters {
        struct_name: String,
        base_reg: UserRegister,
        field_count: u8,
    },
    /// 値を生成しない式 (loop, 配列リテラルなど)
    Void,
}

impl ValueLocation {
    fn register(&self) -> Option<Register> {
        match self {
            ValueLocation::InRegister(r) => Some(*r),
            ValueLocation::InMemory { .. } | ValueLocation::InRegisters { .. } => None,
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

/// ローカル変数のバインディング情報
#[derive(Clone)]
enum LocalBinding {
    /// スカラー値 (u8, bool, enum)
    Single(UserRegister),
    /// struct 値: メモリに格納 (アドレスと struct 名)
    StructInMemory { addr: u16, struct_name: String },
    /// struct 値: 連続レジスタに保持 (base_reg から field_count 個)
    StructInRegisters {
        struct_name: String,
        base_reg: UserRegister,
        field_count: u8,
    },
}

/// 関数ごとの最適化メタデータ (コールグラフ解析で構築)
#[derive(Debug, Clone)]
struct FunctionMeta {
    /// 呼び出すユーザー定義関数のリスト
    callees: HashSet<String>,
    /// 他のユーザー定義関数を呼ばない (callees が空)
    is_leaf: bool,
    /// パラメータのフラット化レジスタ数 (struct レジスタ保持最適化で使用予定)
    #[allow(dead_code)]
    flat_param_count: u8,
    /// 推定最大レジスタ使用数 (caller-save 最適化用)
    estimated_max_reg: u8,
    /// AST ノード数 (インライン展開判定用)
    body_node_count: usize,
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
    /// ミュータブルグローバル変数
    mutable_globals: HashSet<String>,
    /// struct 定義 (名前 → フィールド定義リスト)
    struct_defs: HashMap<String, Vec<StructField>>,
    /// 関数の戻り値型 (struct 戻り値のメモリ化に使用)
    fn_return_types: HashMap<String, Type>,
    /// 関数ごとの最適化メタデータ (コールグラフ解析結果)
    fn_meta: HashMap<String, FunctionMeta>,
    /// 関数の AST 本体 (インライン展開用)
    fn_bodies: HashMap<String, (Vec<Param>, Expr)>,
    /// メモリスロット割り当て用の次のアドレス (struct データ + caller-save 共用)
    next_save_slot: u16,
    /// ローカル変数名 → 割り当て済みバインディング
    local_bindings: HashMap<String, LocalBinding>,
    /// 次に割り当て可能なレジスタ番号
    next_free_reg: u8,
    /// ローカル変数にバインド済みのレジスタ数 (一時レジスタのリセット基準)
    local_var_count: u8,
    /// アドレス未確定の前方参照リスト
    forward_refs: Vec<ForwardRef>,
    /// ループごとの break 先パッチオフセットのスタック
    loop_break_offsets: Vec<Vec<ByteOffset>>,
    /// 現在コード生成中の関数名 (TCO 検出用)
    current_fn_name: Option<String>,
    /// 現在の関数の先頭アドレス (TCO ジャンプ先)
    current_fn_start_addr: Option<Addr>,
    /// 現在の関数のパラメータ数 (TCO 引数コピー用)
    current_fn_param_count: u8,
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
            mutable_globals: HashSet::new(),
            struct_defs: HashMap::new(),
            fn_return_types: HashMap::new(),
            fn_meta: HashMap::new(),
            fn_bodies: HashMap::new(),
            next_save_slot: 0x0A0,
            local_bindings: HashMap::new(),
            next_free_reg: 0,
            local_var_count: 0,
            forward_refs: Vec::new(),
            loop_break_offsets: Vec::new(),
            current_fn_name: None,
            current_fn_start_addr: None,
            current_fn_param_count: 0,
        }
    }

    /// プログラム全体をコード生成し、バイトコードを返す
    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        // Pass 0: enum / struct 定義を登録
        for top in &program.top_levels {
            match top {
                TopLevel::EnumDef { name, variants, .. } => {
                    for (i, variant) in variants.iter().enumerate() {
                        self.enum_variant_values
                            .insert((name.clone(), variant.clone()), i as u8);
                    }
                }
                TopLevel::StructDef { name, fields, .. } => {
                    self.struct_defs.insert(name.clone(), fields.clone());
                }
                _ => {}
            }
        }

        // コールグラフ解析: 関数ごとのメタデータを構築
        self.analyze_call_graph(program);

        // Pass 1: グローバル定数・スプライトをデータとして記録
        for top in &program.top_levels {
            if let TopLevel::LetDef {
                name,
                ty,
                value,
                mutable,
                ..
            } = top
            {
                if *mutable {
                    self.mutable_globals.insert(name.clone());
                }
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
                name,
                params,
                return_type,
                body,
                ..
            } = top
            {
                let addr = self.current_addr();
                self.fn_addrs.insert(name.clone(), addr);
                self.fn_return_types
                    .insert(name.clone(), return_type.clone());

                self.local_bindings.clear();
                self.next_free_reg = 0;
                self.local_var_count = 0;

                // パラメータのフラット合計レジスタ数を計算
                let has_struct_param = params.iter().any(|p| {
                    if let Type::UserType(ref tn) = p.ty {
                        self.struct_defs.contains_key(tn)
                    } else {
                        false
                    }
                });
                let total_flat_params: u8 = params
                    .iter()
                    .map(|p| {
                        if let Type::UserType(ref tn) = p.ty
                            && self.struct_defs.contains_key(tn)
                        {
                            return self.struct_field_count(tn) as u8;
                        }
                        1
                    })
                    .sum();

                // レジスタに余裕がある場合のみ StructInRegisters を使用
                // (パラメータ + let 束縛 + テンプで V14 を超えないようにする)
                const MAX_PARAM_REGS_FOR_REGISTER_MODE: u8 = 4;
                let use_register_mode =
                    !has_struct_param || total_flat_params <= MAX_PARAM_REGS_FOR_REGISTER_MODE;

                if use_register_mode {
                    // レジスタモード: struct パラメータも連続レジスタに直接保持
                    for param in params {
                        if let Type::UserType(ref type_name) = param.ty
                            && self.struct_defs.contains_key(type_name)
                        {
                            let count = self.struct_field_count(type_name) as u8;
                            let base_reg = UserRegister::new(self.next_free_reg);
                            self.next_free_reg += count;
                            self.local_bindings.insert(
                                param.name.clone(),
                                LocalBinding::StructInRegisters {
                                    struct_name: type_name.clone(),
                                    base_reg,
                                    field_count: count,
                                },
                            );
                        } else {
                            let reg = self.alloc_register();
                            self.local_bindings
                                .insert(param.name.clone(), LocalBinding::Single(reg));
                        }
                    }
                } else {
                    // メモリモード: レジスタ圧力が高い場合のフォールバック
                    let params_mem_base = self.alloc_mem_slot(total_flat_params as u16);

                    if total_flat_params > 0 {
                        self.emit_op(Opcode::LdI(Addr::new(params_mem_base)));
                        let last_reg = UserRegister::new(total_flat_params - 1);
                        self.emit_op(Opcode::LdIVx(last_reg.into()));
                    }

                    let mut mem_offset = 0u16;
                    for param in params {
                        if let Type::UserType(ref type_name) = param.ty
                            && self.struct_defs.contains_key(type_name)
                        {
                            let count = self.struct_field_count(type_name) as u16;
                            self.local_bindings.insert(
                                param.name.clone(),
                                LocalBinding::StructInMemory {
                                    addr: params_mem_base + mem_offset,
                                    struct_name: type_name.clone(),
                                },
                            );
                            mem_offset += count;
                        } else {
                            let reg = self.alloc_register();
                            self.emit_load_from_memory(reg.into(), params_mem_base + mem_offset);
                            self.local_bindings
                                .insert(param.name.clone(), LocalBinding::Single(reg));
                            mem_offset += 1;
                        }
                    }
                }
                self.local_var_count = self.next_free_reg;

                // TCO 用に現在の関数情報を記録
                self.current_fn_name = Some(name.clone());
                self.current_fn_start_addr = Some(addr);
                // TCO ジャンプ先は関数先頭なので、フラットなレジスタ数を記録
                self.current_fn_param_count = params
                    .iter()
                    .map(|p| {
                        if let Type::UserType(ref tn) = p.ty
                            && self.struct_defs.contains_key(tn)
                        {
                            return self.struct_field_count(tn) as u8;
                        }
                        1
                    })
                    .sum();

                let result = self.codegen_expr_tail(body);
                // 戻り値を V0..V(n-1) にセットして返す
                match &result {
                    ValueLocation::InMemory {
                        addr: mem_addr,
                        struct_name,
                    } => {
                        let count = self.struct_field_count(struct_name);
                        self.emit_op(Opcode::LdI(Addr::new(*mem_addr)));
                        let last = UserRegister::new(count as u8 - 1);
                        self.emit_op(Opcode::LdVxI(last.into()));
                    }
                    ValueLocation::InRegisters {
                        base_reg,
                        field_count,
                        ..
                    } => {
                        // レジスタ上の struct を V0..V(n-1) にコピー
                        for i in 0..*field_count {
                            let src = UserRegister::new(base_reg.index() + i);
                            let dst = UserRegister::new(i);
                            if src != dst {
                                self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                            }
                        }
                    }
                    _ => {
                        if let Some(reg) = result.register()
                            && reg != Register::V0
                        {
                            self.emit_op(Opcode::LdReg(Register::V0, reg));
                        }
                    }
                }
                if self.current_fn_name.as_deref() == Some("main") {
                    // main は JP で呼ばれるため RET ではなくセルフループで停止
                    let halt_addr = self.current_addr();
                    self.emit_op(Opcode::Jp(halt_addr));
                } else {
                    self.emit_op(Opcode::Ret);
                }

                self.current_fn_name = None;
                self.current_fn_start_addr = None;
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

    fn lookup_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.local_bindings.get(name)
    }

    fn v0_is_bound(&self) -> bool {
        self.next_free_reg > 0
    }

    /// struct のフラット化フィールド数を計算
    fn struct_field_count(&self, struct_name: &str) -> usize {
        if let Some(fields) = self.struct_defs.get(struct_name) {
            fields
                .iter()
                .map(|f| {
                    if let Type::UserType(ref name) = f.ty
                        && self.struct_defs.contains_key(name)
                    {
                        return self.struct_field_count(name);
                    }
                    1
                })
                .sum()
        } else {
            1
        }
    }

    /// struct 内のフィールドのレジスタオフセットを計算
    fn struct_field_offset(&self, struct_name: &str, field_name: &str) -> Option<usize> {
        let fields = self.struct_defs.get(struct_name)?;
        let mut offset = 0;
        for f in fields {
            if f.name == field_name {
                return Some(offset);
            }
            if let Type::UserType(ref name) = f.ty
                && self.struct_defs.contains_key(name)
            {
                offset += self.struct_field_count(name);
                continue;
            }
            offset += 1;
        }
        None
    }

    /// struct のフィールドの型を取得
    fn struct_field_type(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        let fields = self.struct_defs.get(struct_name)?;
        fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| f.ty.clone())
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

    fn emit_global_write(&mut self, name: &str, src_reg: Register) {
        if src_reg == Register::V0 {
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdIVx(Register::V0));
        } else if self.v0_is_bound() {
            // V0 を退避して値を書き込み
            self.emit_op(Opcode::Xor(src_reg, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, src_reg));
            self.emit_op(Opcode::Xor(src_reg, Register::V0));
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdIVx(Register::V0));
            // V0 を復帰
            self.emit_op(Opcode::Xor(src_reg, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, src_reg));
            self.emit_op(Opcode::Xor(src_reg, Register::V0));
        } else {
            self.emit_op(Opcode::LdReg(Register::V0, src_reg));
            self.emit_ld_i_global(name);
            self.emit_op(Opcode::LdIVx(Register::V0));
        }
    }

    /// メモリスロットを割り当て、開始アドレスを返す
    fn alloc_mem_slot(&mut self, size: u16) -> u16 {
        let addr = self.next_save_slot;
        self.next_save_slot += size;
        addr
    }

    /// メモリからレジスタに 1 バイトをロード (V0 bounce パターン)
    fn emit_load_from_memory(&mut self, target: Register, addr: u16) {
        self.emit_op(Opcode::LdI(Addr::new(addr)));
        if target == Register::V0 {
            self.emit_op(Opcode::LdVxI(Register::V0));
        } else if self.v0_is_bound() {
            // V0 がバインド済み: XOR swap パターン
            self.emit_op(Opcode::LdReg(target, Register::V0));
            self.emit_op(Opcode::LdVxI(Register::V0));
            self.emit_op(Opcode::Xor(target, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, target));
            self.emit_op(Opcode::Xor(target, Register::V0));
        } else {
            self.emit_op(Opcode::LdVxI(Register::V0));
            self.emit_op(Opcode::LdReg(target, Register::V0));
        }
    }

    /// レジスタの値をメモリに 1 バイト書き込む
    fn emit_store_to_memory(&mut self, src: Register, addr: u16) {
        if src == Register::V0 {
            self.emit_op(Opcode::LdI(Addr::new(addr)));
            self.emit_op(Opcode::LdIVx(Register::V0));
        } else if self.v0_is_bound() {
            // V0 がバインド済み: XOR swap で V0 と src を入れ替え → 書き込み → 戻す
            self.emit_op(Opcode::Xor(src, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, src));
            self.emit_op(Opcode::Xor(src, Register::V0));
            self.emit_op(Opcode::LdI(Addr::new(addr)));
            self.emit_op(Opcode::LdIVx(Register::V0));
            // swap back
            self.emit_op(Opcode::Xor(src, Register::V0));
            self.emit_op(Opcode::Xor(Register::V0, src));
            self.emit_op(Opcode::Xor(src, Register::V0));
        } else {
            self.emit_op(Opcode::LdReg(Register::V0, src));
            self.emit_op(Opcode::LdI(Addr::new(addr)));
            self.emit_op(Opcode::LdIVx(Register::V0));
        }
    }

    /// メモリ間コピー (レジスタを破壊しない)
    /// emit_load_from_memory / emit_store_to_memory の XOR swap により
    /// V0 を含む全ライブレジスタを保護する
    fn emit_memcpy(&mut self, src_addr: u16, dst_addr: u16, count: usize) {
        let tmp = self.alloc_temp_register();
        for i in 0..count {
            self.emit_load_from_memory(tmp.into(), src_addr + i as u16);
            self.emit_store_to_memory(tmp.into(), dst_addr + i as u16);
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

    /// match の arm パターンが 0 始まりの連続整数かどうかを判定
    fn can_use_jump_table(&self, arms: &[MatchArm]) -> bool {
        if arms.len() < 2 {
            return false;
        }
        let mut values: Vec<u8> = arms
            .iter()
            .map(|arm| self.pattern_value(&arm.pattern))
            .collect();
        values.sort();
        values.iter().enumerate().all(|(i, &v)| v == i as u8)
    }

    /// ジャンプテーブルを使った match コード生成
    fn codegen_match_jump_table(&mut self, scr_reg: Register, arms: &[MatchArm]) -> ValueLocation {
        // arm をパターン値でソート (ジャンプテーブルのインデックス順)
        let mut sorted_arms: Vec<(u8, &MatchArm)> = arms
            .iter()
            .map(|arm| (self.pattern_value(&arm.pattern), arm))
            .collect();
        sorted_arms.sort_by_key(|(val, _)| *val);

        // scrutinee を V0 にコピー (V0 でなければ)
        if scr_reg != Register::V0 {
            self.emit_op(Opcode::LdReg(Register::V0, scr_reg));
        }
        // V0 *= 2 (各ジャンプテーブルエントリ = 1 JP = 2 bytes)
        self.emit_op(Opcode::Add(Register::V0, Register::V0));
        // JpV0(table_base): table_base は次の命令のアドレス
        let table_base = Addr::new(self.current_addr().raw() + INSTRUCTION_SIZE);
        self.emit_op(Opcode::JpV0(table_base));

        // ジャンプテーブル: 各 arm への JP placeholder
        let table_placeholders: Vec<ByteOffset> = (0..sorted_arms.len())
            .map(|_| self.emit_placeholder())
            .collect();

        // 各 arm の body を生成
        let mut end_offsets = Vec::new();
        let last_idx = sorted_arms.len() - 1;
        let mut result_loc = ValueLocation::Void;

        for (i, (_, arm)) in sorted_arms.iter().enumerate() {
            // ジャンプテーブルエントリをパッチ
            let arm_addr = self.current_addr();
            self.patch_at(table_placeholders[i], Opcode::Jp(arm_addr));

            // arm body を生成
            let loc = self.codegen_expr(&arm.body);

            if i == last_idx {
                // 最終 arm: end ジャンプ不要
                result_loc = loc;
                let end_addr = self.current_addr();
                for off in &end_offsets {
                    self.patch_at(*off, Opcode::Jp(end_addr));
                }
            } else {
                // 中間 arm: end へのジャンプ
                let jp_end = self.emit_placeholder();
                end_offsets.push(jp_end);
            }
        }

        result_loc
    }

    // ---- コールグラフ解析 ----

    /// AST を走査して関数ごとのメタデータを構築する
    fn analyze_call_graph(&mut self, program: &Program) {
        // 全関数名を収集 (ユーザー定義関数の判定用)
        let fn_names: HashSet<String> = program
            .top_levels
            .iter()
            .filter_map(|top| {
                if let TopLevel::FnDef { name, .. } = top {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        for top in &program.top_levels {
            if let TopLevel::FnDef {
                name, params, body, ..
            } = top
            {
                let mut callees = HashSet::new();
                Self::collect_callees(body, &fn_names, &mut callees);

                let flat_param_count: u8 = params
                    .iter()
                    .map(|p| {
                        if let Type::UserType(ref tn) = p.ty
                            && let Some(fields) = self.struct_defs.get(tn)
                        {
                            return fields.len() as u8;
                        }
                        1
                    })
                    .sum();

                let let_count = Self::count_let_bindings(body);
                let expr_temps = Self::estimate_expr_temps(body);
                // 関数が使うレジスタ: パラメータ + let束縛 + 式評価テンポラリ
                let estimated_max_reg = (flat_param_count + let_count + expr_temps).min(15);

                let body_node_count = Self::count_ast_nodes(body);
                let is_leaf = callees.is_empty();
                self.fn_meta.insert(
                    name.clone(),
                    FunctionMeta {
                        callees,
                        is_leaf,
                        flat_param_count,
                        estimated_max_reg,
                        body_node_count,
                    },
                );
                // インライン展開用に関数本体を保存
                self.fn_bodies
                    .insert(name.clone(), (params.clone(), body.clone()));
            }
        }
    }

    /// インライン展開すべきかどうかを判定
    fn should_inline(&self, name: &str) -> bool {
        if name == "main" {
            return false;
        }
        let Some(meta) = self.fn_meta.get(name) else {
            return false;
        };
        // 自己再帰なし && 小さな本体
        // リーフ関数 or callee が 1 つだけの非リーフ関数をインライン対象にする
        !meta.callees.contains(name)
            && meta.body_node_count <= 30
            && (meta.is_leaf || meta.callees.len() == 1)
    }

    /// 式中のユーザー定義関数呼び出しを再帰的に収集
    fn collect_callees(expr: &Expr, fn_names: &HashSet<String>, out: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::Call { name, args } => {
                if fn_names.contains(name) {
                    out.insert(name.clone());
                }
                for arg in args {
                    Self::collect_callees(arg, fn_names, out);
                }
            }
            ExprKind::BuiltinCall { args, .. } => {
                for arg in args {
                    Self::collect_callees(arg, fn_names, out);
                }
            }
            ExprKind::BinaryOp { lhs, rhs, .. } => {
                Self::collect_callees(lhs, fn_names, out);
                Self::collect_callees(rhs, fn_names, out);
            }
            ExprKind::UnaryOp { expr, .. } => {
                Self::collect_callees(expr, fn_names, out);
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                Self::collect_callees(cond, fn_names, out);
                Self::collect_callees(then_block, fn_names, out);
                if let Some(e) = else_block {
                    Self::collect_callees(e, fn_names, out);
                }
            }
            ExprKind::Loop { body } => {
                Self::collect_callees(body, fn_names, out);
            }
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    Self::collect_callees_stmt(stmt, fn_names, out);
                }
                if let Some(e) = expr {
                    Self::collect_callees(e, fn_names, out);
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                Self::collect_callees(scrutinee, fn_names, out);
                for arm in arms {
                    Self::collect_callees(&arm.body, fn_names, out);
                }
            }
            ExprKind::StructLiteral { fields, base, .. } => {
                for (_, expr) in fields {
                    Self::collect_callees(expr, fn_names, out);
                }
                if let Some(b) = base {
                    Self::collect_callees(b, fn_names, out);
                }
            }
            ExprKind::FieldAccess { expr, .. } => {
                Self::collect_callees(expr, fn_names, out);
            }
            ExprKind::Index { array, index } => {
                Self::collect_callees(array, fn_names, out);
                Self::collect_callees(index, fn_names, out);
            }
            ExprKind::ArrayLiteral(elems) => {
                for elem in elems {
                    Self::collect_callees(elem, fn_names, out);
                }
            }
            ExprKind::IntLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::Ident(_)
            | ExprKind::EnumVariant { .. } => {}
        }
    }

    /// 文中のユーザー定義関数呼び出しを再帰的に収集
    fn collect_callees_stmt(stmt: &Stmt, fn_names: &HashSet<String>, out: &mut HashSet<String>) {
        match &stmt.kind {
            StmtKind::Let { value, .. } => Self::collect_callees(value, fn_names, out),
            StmtKind::Assign { value, .. } => Self::collect_callees(value, fn_names, out),
            StmtKind::IndexAssign { index, value, .. } => {
                Self::collect_callees(index, fn_names, out);
                Self::collect_callees(value, fn_names, out);
            }
            StmtKind::Expr(expr) => Self::collect_callees(expr, fn_names, out),
            StmtKind::Return(Some(expr)) => Self::collect_callees(expr, fn_names, out),
            StmtKind::Return(None) | StmtKind::Break => {}
        }
    }

    /// AST ノード数を再帰的に数える (インライン判定用)
    fn count_ast_nodes(expr: &Expr) -> usize {
        match &expr.kind {
            ExprKind::IntLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::Ident(_)
            | ExprKind::EnumVariant { .. } => 1,
            ExprKind::BinaryOp { lhs, rhs, .. } => {
                1 + Self::count_ast_nodes(lhs) + Self::count_ast_nodes(rhs)
            }
            ExprKind::UnaryOp { expr, .. } => 1 + Self::count_ast_nodes(expr),
            ExprKind::Call { args, .. } | ExprKind::BuiltinCall { args, .. } => {
                1 + args.iter().map(Self::count_ast_nodes).sum::<usize>()
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                1 + Self::count_ast_nodes(cond)
                    + Self::count_ast_nodes(then_block)
                    + else_block
                        .as_ref()
                        .map(|e| Self::count_ast_nodes(e))
                        .unwrap_or(0)
            }
            ExprKind::Loop { body } => 1 + Self::count_ast_nodes(body),
            ExprKind::Block { stmts, expr } => {
                1 + stmts.iter().map(Self::count_ast_nodes_stmt).sum::<usize>()
                    + expr.as_ref().map(|e| Self::count_ast_nodes(e)).unwrap_or(0)
            }
            ExprKind::Match { scrutinee, arms } => {
                1 + Self::count_ast_nodes(scrutinee)
                    + arms
                        .iter()
                        .map(|a| 1 + Self::count_ast_nodes(&a.body))
                        .sum::<usize>()
            }
            ExprKind::StructLiteral { fields, base, .. } => {
                1 + fields
                    .iter()
                    .map(|(_, e)| Self::count_ast_nodes(e))
                    .sum::<usize>()
                    + base.as_ref().map(|e| Self::count_ast_nodes(e)).unwrap_or(0)
            }
            ExprKind::FieldAccess { expr, .. } => 1 + Self::count_ast_nodes(expr),
            ExprKind::Index { array, index } => {
                1 + Self::count_ast_nodes(array) + Self::count_ast_nodes(index)
            }
            ExprKind::ArrayLiteral(elems) => {
                1 + elems.iter().map(Self::count_ast_nodes).sum::<usize>()
            }
        }
    }

    fn count_ast_nodes_stmt(stmt: &Stmt) -> usize {
        match &stmt.kind {
            StmtKind::Let { value, .. } => 1 + Self::count_ast_nodes(value),
            StmtKind::Assign { value, .. } => 1 + Self::count_ast_nodes(value),
            StmtKind::IndexAssign { index, value, .. } => {
                1 + Self::count_ast_nodes(index) + Self::count_ast_nodes(value)
            }
            StmtKind::Expr(expr) => Self::count_ast_nodes(expr),
            StmtKind::Return(Some(expr)) => 1 + Self::count_ast_nodes(expr),
            StmtKind::Return(None) | StmtKind::Break => 1,
        }
    }

    /// 式中の let 束縛の数を数える
    fn count_let_bindings(expr: &Expr) -> u8 {
        match &expr.kind {
            ExprKind::Block { stmts, expr } => {
                let mut count: u8 = 0;
                for stmt in stmts {
                    if matches!(stmt.kind, StmtKind::Let { .. }) {
                        count = count.saturating_add(1);
                    }
                    count = count.saturating_add(Self::count_let_bindings_stmt(stmt));
                }
                if let Some(e) = expr {
                    count = count.saturating_add(Self::count_let_bindings(e));
                }
                count
            }
            ExprKind::If {
                then_block,
                else_block,
                ..
            } => {
                let a = Self::count_let_bindings(then_block);
                let b = else_block
                    .as_ref()
                    .map(|e| Self::count_let_bindings(e))
                    .unwrap_or(0);
                a.max(b)
            }
            ExprKind::Loop { body } => Self::count_let_bindings(body),
            ExprKind::Match { arms, .. } => arms
                .iter()
                .map(|arm| Self::count_let_bindings(&arm.body))
                .max()
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn count_let_bindings_stmt(stmt: &Stmt) -> u8 {
        match &stmt.kind {
            StmtKind::Expr(expr) => Self::count_let_bindings(expr),
            _ => 0,
        }
    }

    /// 式の評価中に割り当てられるテンポラリレジスタ数を推定
    /// (codegen の alloc_temp_register 呼び出し回数の上界)
    fn estimate_expr_temps(expr: &Expr) -> u8 {
        match &expr.kind {
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) | ExprKind::EnumVariant { .. } => 1,
            ExprKind::Ident(_) => 0,
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let base =
                    Self::estimate_expr_temps(lhs).saturating_add(Self::estimate_expr_temps(rhs));
                let extra = match op {
                    BinOp::Add | BinOp::Sub | BinOp::And | BinOp::Or => 0,
                    BinOp::Mul | BinOp::Div => 3,
                    BinOp::Mod => 1,
                    BinOp::Eq | BinOp::NotEq => 1,
                    BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => 2,
                };
                base.saturating_add(extra)
            }
            ExprKind::UnaryOp { expr, .. } => Self::estimate_expr_temps(expr).saturating_add(1),
            ExprKind::Call { args, .. } | ExprKind::BuiltinCall { args, .. } => {
                let arg_temps: u8 = args.iter().map(Self::estimate_expr_temps).sum();
                arg_temps.saturating_add(1)
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let c = Self::estimate_expr_temps(cond);
                let t = Self::estimate_expr_temps(then_block);
                let e = else_block
                    .as_ref()
                    .map(|e| Self::estimate_expr_temps(e))
                    .unwrap_or(0);
                c.max(t).max(e)
            }
            ExprKind::Block { stmts, expr } => {
                let stmt_max = stmts
                    .iter()
                    .map(Self::estimate_stmt_temps)
                    .max()
                    .unwrap_or(0);
                let expr_max = expr
                    .as_ref()
                    .map(|e| Self::estimate_expr_temps(e))
                    .unwrap_or(0);
                stmt_max.max(expr_max)
            }
            ExprKind::Loop { body } => Self::estimate_expr_temps(body),
            ExprKind::Match { scrutinee, arms } => {
                let s = Self::estimate_expr_temps(scrutinee);
                let arm_max = arms
                    .iter()
                    .map(|a| Self::estimate_expr_temps(&a.body))
                    .max()
                    .unwrap_or(0);
                s.max(arm_max)
            }
            ExprKind::StructLiteral { fields, base, .. } => {
                let f: u8 = fields
                    .iter()
                    .map(|(_, e)| Self::estimate_expr_temps(e))
                    .sum();
                let b = base
                    .as_ref()
                    .map(|e| Self::estimate_expr_temps(e))
                    .unwrap_or(0);
                f.max(b)
            }
            ExprKind::FieldAccess { expr, .. } => Self::estimate_expr_temps(expr),
            ExprKind::Index { array, index } => {
                Self::estimate_expr_temps(array).saturating_add(Self::estimate_expr_temps(index))
            }
            ExprKind::ArrayLiteral(elems) => elems.iter().map(Self::estimate_expr_temps).sum(),
        }
    }

    fn estimate_stmt_temps(stmt: &Stmt) -> u8 {
        match &stmt.kind {
            StmtKind::Let { value, .. } => Self::estimate_expr_temps(value),
            StmtKind::Assign { value, .. } => Self::estimate_expr_temps(value),
            StmtKind::IndexAssign { index, value, .. } => {
                Self::estimate_expr_temps(index).saturating_add(Self::estimate_expr_temps(value))
            }
            StmtKind::Expr(expr) => Self::estimate_expr_temps(expr),
            StmtKind::Return(Some(expr)) => Self::estimate_expr_temps(expr),
            StmtKind::Return(None) | StmtKind::Break => 0,
        }
    }

    // ---- コード生成 ----

    /// 関数をインライン展開してコード生成
    ///
    /// CALL/RET + caller-save/restore を完全に省略し、
    /// callee の本体を caller のコンテキストに直接埋め込む。
    fn codegen_inline_call(&mut self, name: &str, args: &[Expr]) -> ValueLocation {
        let (params, body) = self.fn_bodies.get(name).unwrap().clone();

        // 1. 引数を評価 (caller のコンテキストで)
        let mut arg_locs: Vec<(String, Type, ValueLocation)> = Vec::new();
        for (param, arg) in params.iter().zip(args.iter()) {
            let loc = self.codegen_expr(arg);
            arg_locs.push((param.name.clone(), param.ty.clone(), loc));
        }

        // 2. caller の状態を保存
        let saved_bindings = self.local_bindings.clone();
        let saved_next_free_reg = self.next_free_reg;
        let saved_local_var_count = self.local_var_count;

        // 3. callee のパラメータをバインド
        self.local_bindings.clear();
        // next_free_reg はリセットせず、caller のテンポラリ領域の続きを使う
        // local_var_count は callee のバインディング数に設定
        self.local_var_count = self.next_free_reg;

        for (param_name, param_ty, loc) in arg_locs {
            let is_struct = if let Type::UserType(ref tn) = param_ty {
                self.struct_defs.contains_key(tn)
            } else {
                false
            };

            if is_struct {
                match loc {
                    ValueLocation::InMemory { addr, struct_name } => {
                        self.local_bindings.insert(
                            param_name,
                            LocalBinding::StructInMemory { addr, struct_name },
                        );
                    }
                    ValueLocation::InRegisters {
                        struct_name,
                        base_reg,
                        field_count,
                    } => {
                        // レジスタ上の struct をコピーしてバインド
                        let new_base = UserRegister::new(self.next_free_reg);
                        for i in 0..field_count {
                            let src = UserRegister::new(base_reg.index() + i);
                            let dst = UserRegister::new(self.next_free_reg + i);
                            if src != dst {
                                self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                            }
                        }
                        self.next_free_reg += field_count;
                        self.local_bindings.insert(
                            param_name,
                            LocalBinding::StructInRegisters {
                                struct_name,
                                base_reg: new_base,
                                field_count,
                            },
                        );
                    }
                    ValueLocation::InRegister(reg) => {
                        let new_reg = self.alloc_register();
                        if Register::from(new_reg) != reg {
                            self.emit_op(Opcode::LdReg(new_reg.into(), reg));
                        }
                        self.local_bindings
                            .insert(param_name, LocalBinding::Single(new_reg));
                    }
                    ValueLocation::Void => {}
                }
            } else if let ValueLocation::InRegister(reg) = loc {
                // スカラー引数: 新しいレジスタにコピーして callee のパラメータとしてバインド
                let new_reg = self.alloc_register();
                if Register::from(new_reg) != reg {
                    self.emit_op(Opcode::LdReg(new_reg.into(), reg));
                }
                self.local_bindings
                    .insert(param_name, LocalBinding::Single(new_reg));
            }
        }
        self.local_var_count = self.next_free_reg;

        // 4. callee の本体をインラインでコード生成
        let result = self.codegen_expr(&body);

        // 5. 結果を caller のテンポラリとして保持
        let final_result = match result {
            ValueLocation::InRegister(reg) => ValueLocation::InRegister(reg),
            ValueLocation::InMemory { addr, struct_name } => {
                ValueLocation::InMemory { addr, struct_name }
            }
            ValueLocation::InRegisters {
                struct_name,
                base_reg,
                field_count,
            } => ValueLocation::InRegisters {
                struct_name,
                base_reg,
                field_count,
            },
            ValueLocation::Void => ValueLocation::Void,
        };

        // 6. caller の状態を復帰 (ただし next_free_reg は結果レジスタを含む値に)
        self.local_bindings = saved_bindings;
        self.local_var_count = saved_local_var_count;
        // next_free_reg は callee が使ったレジスタを含めて維持
        // (結果レジスタが caller の既存レジスタと重ならないようにするため)
        // ただし、caller の元の next_free_reg より小さくはしない
        self.next_free_reg = self.next_free_reg.max(saved_next_free_reg);

        final_result
    }

    /// 末尾位置の式をコード生成 (TCO 対象の自己再帰を検出)
    fn codegen_expr_tail(&mut self, expr: &Expr) -> ValueLocation {
        match &expr.kind {
            // 末尾位置での自己再帰呼び出し → TCO
            ExprKind::Call { name, args } if self.current_fn_name.as_deref() == Some(name) => {
                let fn_start = self.current_fn_start_addr.unwrap();
                let param_count = self.current_fn_param_count;

                // 全引数をフラットなレジスタリストに評価
                let mut flat_args: Vec<Register> = Vec::new();
                for arg in args {
                    let loc = self.codegen_expr(arg);
                    match loc {
                        ValueLocation::InMemory {
                            addr,
                            ref struct_name,
                        } => {
                            let count = self.struct_field_count(struct_name);
                            for i in 0..count {
                                let reg = self.alloc_temp_register();
                                self.emit_load_from_memory(reg.into(), addr + i as u16);
                                flat_args.push(reg.into());
                            }
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..field_count {
                                flat_args.push(UserRegister::new(base_reg.index() + i).into());
                            }
                        }
                        _ => {
                            if let Some(reg) = loc.register() {
                                flat_args.push(reg);
                            }
                        }
                    }
                }

                // flat_args → V0, V1, ... にコピー (パラメータ上書き)
                for i in 0..param_count {
                    let target: Register = UserRegister::new(i).into();
                    if (i as usize) < flat_args.len() && flat_args[i as usize] != target {
                        self.emit_op(Opcode::LdReg(target, flat_args[i as usize]));
                    }
                }

                // 関数先頭にジャンプ (CALL + RET の代わり)
                self.emit_op(Opcode::Jp(fn_start));
                ValueLocation::Void
            }
            // if-else: 両ブランチを末尾位置として再帰
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                // 条件式テンプ割り当て前に保存 (テンプをブランチで再利用可能にする)
                let saved_local_var_count = self.local_var_count;
                let saved_next_free_reg = self.next_free_reg;

                let Some(cond_reg) = self.codegen_expr(cond).register() else {
                    return ValueLocation::Void;
                };
                self.emit_op(Opcode::SneImm(cond_reg, 0x00));
                let jp_else_offset = self.emit_placeholder();

                let then_loc = self.codegen_expr_tail(then_block);

                if let Some(else_block) = else_block {
                    // then ブランチの struct → V0..V(n-1) にロード
                    let then_is_struct = matches!(
                        &then_loc,
                        ValueLocation::InMemory { .. } | ValueLocation::InRegisters { .. }
                    );
                    match &then_loc {
                        ValueLocation::InMemory { addr, struct_name } => {
                            let count = self.struct_field_count(struct_name);
                            self.emit_op(Opcode::LdI(Addr::new(*addr)));
                            let last = UserRegister::new(count as u8 - 1);
                            self.emit_op(Opcode::LdVxI(last.into()));
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..*field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        _ => {}
                    }

                    let jp_end_offset = self.emit_placeholder();

                    let else_addr = self.current_addr();
                    self.patch_at(jp_else_offset, Opcode::Jp(else_addr));

                    // else ブランチでは then のレジスタ状態をリセット
                    self.local_var_count = saved_local_var_count;
                    self.next_free_reg = saved_next_free_reg;

                    let else_loc = self.codegen_expr_tail(else_block);

                    // else ブランチの struct → V0..V(n-1) にロード
                    let else_is_struct = matches!(
                        &else_loc,
                        ValueLocation::InMemory { .. } | ValueLocation::InRegisters { .. }
                    );
                    match &else_loc {
                        ValueLocation::InMemory { addr, struct_name } => {
                            let count = self.struct_field_count(struct_name);
                            self.emit_op(Opcode::LdI(Addr::new(*addr)));
                            let last = UserRegister::new(count as u8 - 1);
                            self.emit_op(Opcode::LdVxI(last.into()));
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..*field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        _ => {}
                    }

                    let end_addr = self.current_addr();
                    self.patch_at(jp_end_offset, Opcode::Jp(end_addr));

                    match (then_loc, else_loc) {
                        _ if then_is_struct || else_is_struct => {
                            ValueLocation::InRegister(Register::V0)
                        }
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
            // block: 末尾式を末尾位置として再帰
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    self.codegen_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.codegen_expr_tail(tail)
                } else {
                    ValueLocation::Void
                }
            }
            // その他: 通常のコード生成にフォールバック
            _ => self.codegen_expr(expr),
        }
    }

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
                if let Some(binding) = self.lookup_binding(name).cloned() {
                    match binding {
                        LocalBinding::Single(reg) => ValueLocation::InRegister(reg.into()),
                        LocalBinding::StructInMemory {
                            addr,
                            ref struct_name,
                        } => ValueLocation::InMemory {
                            addr,
                            struct_name: struct_name.clone(),
                        },
                        LocalBinding::StructInRegisters {
                            ref struct_name,
                            base_reg,
                            field_count,
                        } => ValueLocation::InRegisters {
                            struct_name: struct_name.clone(),
                            base_reg,
                            field_count,
                        },
                    }
                } else {
                    let reg = self.alloc_temp_register();
                    if self.data_offsets.contains_key(name) {
                        self.emit_global_read(reg.into(), name);
                    }
                    ValueLocation::InRegister(reg.into())
                }
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                // struct の等値比較: フィールドごとに比較
                if matches!(op, BinOp::Eq | BinOp::NotEq) {
                    let lhs_loc = self.codegen_expr(lhs);
                    let rhs_loc = self.codegen_expr(rhs);

                    // struct 同士の比較 (InMemory / InRegisters)
                    if let (
                        ValueLocation::InMemory {
                            addr: l_addr,
                            struct_name: l_name,
                        },
                        ValueLocation::InMemory {
                            addr: r_addr,
                            struct_name: r_name,
                        },
                    ) = (&lhs_loc, &rhs_loc)
                    {
                        return self.codegen_struct_equality_memory(
                            *l_addr,
                            l_name,
                            *r_addr,
                            r_name,
                            *op == BinOp::Eq,
                        );
                    }
                    // InRegisters 同士の比較
                    if let (
                        ValueLocation::InRegisters {
                            base_reg: l_base,
                            field_count: l_count,
                            ..
                        },
                        ValueLocation::InRegisters {
                            base_reg: r_base, ..
                        },
                    ) = (&lhs_loc, &rhs_loc)
                    {
                        return self.codegen_struct_equality_registers(
                            *l_base,
                            *l_count,
                            *r_base,
                            *op == BinOp::Eq,
                        );
                    }

                    // スカラー比較にフォールスルー
                    let Some(lhs_reg) = lhs_loc.register() else {
                        return ValueLocation::Void;
                    };
                    let Some(rhs_reg) = rhs_loc.register() else {
                        return ValueLocation::Void;
                    };
                    let res = self.alloc_temp_register();
                    if *op == BinOp::Eq {
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SeReg(lhs_reg, rhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                    } else {
                        self.emit_op(Opcode::LdImm(res.into(), 0));
                        self.emit_op(Opcode::SneReg(lhs_reg, rhs_reg));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        self.emit_op(Opcode::LdImm(res.into(), 1));
                    }
                    return ValueLocation::InRegister(res.into());
                }

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
                    BinOp::Mul => {
                        // ソフトウェア乗算: result += lhs を rhs 回繰り返す
                        let result = self.alloc_temp_register();
                        let counter = self.alloc_temp_register();
                        let one = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(result.into(), 0));
                        self.emit_op(Opcode::LdReg(counter.into(), rhs_reg));
                        self.emit_op(Opcode::LdImm(one.into(), 1));
                        let loop_addr = self.current_addr();
                        // counter == 0 なら終了
                        self.emit_op(Opcode::SeImm(counter.into(), 0));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        let jp_break = self.emit_placeholder();
                        self.emit_op(Opcode::Add(result.into(), lhs_reg));
                        self.emit_op(Opcode::Sub(counter.into(), one.into()));
                        self.emit_op(Opcode::Jp(loop_addr));
                        let break_addr = self.current_addr();
                        self.patch_at(jp_break, Opcode::Jp(break_addr));
                        return ValueLocation::InRegister(result.into());
                    }
                    BinOp::Div => {
                        // ソフトウェア除算: lhs から rhs を引ける回数を数える
                        let quotient = self.alloc_temp_register();
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(quotient.into(), 0));
                        self.emit_op(Opcode::LdReg(tmp.into(), lhs_reg));
                        let loop_addr = self.current_addr();
                        // tmp -= rhs, VF=1 なら borrow なし (tmp >= rhs)
                        self.emit_op(Opcode::Sub(tmp.into(), rhs_reg));
                        // VF == 0 (borrow) なら終了
                        self.emit_op(Opcode::SneImm(Register::VF, 0));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        let jp_break = self.emit_placeholder();
                        let one = self.alloc_temp_register();
                        self.emit_op(Opcode::LdImm(one.into(), 1));
                        self.emit_op(Opcode::Add(quotient.into(), one.into()));
                        self.emit_op(Opcode::Jp(loop_addr));
                        let break_addr = self.current_addr();
                        self.patch_at(jp_break, Opcode::Jp(break_addr));
                        return ValueLocation::InRegister(quotient.into());
                    }
                    BinOp::Mod => {
                        // ソフトウェア剰余: lhs から rhs を引き続け、残りを返す
                        let tmp = self.alloc_temp_register();
                        self.emit_op(Opcode::LdReg(tmp.into(), lhs_reg));
                        let loop_addr = self.current_addr();
                        self.emit_op(Opcode::Sub(tmp.into(), rhs_reg));
                        // VF == 0 (borrow) なら tmp < rhs だった → 戻して終了
                        self.emit_op(Opcode::SneImm(Register::VF, 0));
                        self.emit_op(Opcode::Jp(self.skip_next_addr()));
                        let jp_break = self.emit_placeholder();
                        self.emit_op(Opcode::Jp(loop_addr));
                        let break_addr = self.current_addr();
                        self.patch_at(jp_break, Opcode::Jp(break_addr));
                        // SUB で壊れた tmp に rhs を足し戻す
                        self.emit_op(Opcode::Add(tmp.into(), rhs_reg));
                        return ValueLocation::InRegister(tmp.into());
                    }
                    // Eq/NotEq は上で早期リターン済み
                    BinOp::Eq | BinOp::NotEq => unreachable!(),
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
            ExprKind::Call { name, args } if self.should_inline(name) => {
                self.codegen_inline_call(name, args)
            }
            ExprKind::Call { name, args } => {
                // ユーザー定義関数: 引数を評価してフラットなレジスタリストを構築
                let mut flat_args: Vec<Register> = Vec::new();
                for arg in args {
                    let loc = self.codegen_expr(arg);
                    match loc {
                        ValueLocation::InMemory {
                            addr,
                            ref struct_name,
                        } => {
                            // struct 引数: メモリからフィールドを1つずつロード
                            let count = self.struct_field_count(struct_name);
                            for i in 0..count {
                                let reg = self.alloc_temp_register();
                                self.emit_load_from_memory(reg.into(), addr + i as u16);
                                flat_args.push(reg.into());
                            }
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            // struct 引数: レジスタから直接フラット化
                            for i in 0..field_count {
                                flat_args.push(UserRegister::new(base_reg.index() + i).into());
                            }
                        }
                        _ => {
                            if let Some(reg) = loc.register() {
                                flat_args.push(reg);
                            }
                        }
                    }
                }
                // caller-save: ライブレジスタをメモリに退避
                // リーフ関数の場合、callee の推定最大レジスタ使用数に制限して
                // 退避レジスタ数を削減する (callee が触らないレジスタは保護不要)
                let num_to_save = if let Some(meta) = self.fn_meta.get(name) {
                    if meta.is_leaf {
                        self.next_free_reg.min(meta.estimated_max_reg)
                    } else {
                        self.next_free_reg
                    }
                } else {
                    self.next_free_reg
                };

                let save_addr = self.next_save_slot;
                if num_to_save > 0 {
                    self.emit_op(Opcode::LdI(Addr::new(save_addr)));
                    let last_reg = UserRegister::new(num_to_save - 1);
                    self.emit_op(Opcode::LdIVx(last_reg.into()));
                    self.next_save_slot += num_to_save as u16;
                }

                // 引数を V0, V1, ... にコピー
                for (i, &arg_reg) in flat_args.iter().enumerate() {
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

                // 戻り値が struct 型かチェック
                let return_type = self.fn_return_types.get(name).cloned();
                let is_struct_return = if let Some(Type::UserType(ref tn)) = return_type {
                    self.struct_defs.contains_key(tn)
                } else {
                    false
                };

                if is_struct_return {
                    let ret_struct_name = if let Some(Type::UserType(ref tn)) = return_type {
                        tn.clone()
                    } else {
                        unreachable!()
                    };
                    let ret_count = self.struct_field_count(&ret_struct_name);

                    // 戻り値 (V0..V(n-1)) をメモリに保存
                    let ret_addr = self.alloc_mem_slot(ret_count as u16);

                    // caller-save の復帰前に struct 戻り値をメモリに退避
                    self.emit_op(Opcode::LdI(Addr::new(ret_addr)));
                    let last = UserRegister::new(ret_count as u8 - 1);
                    self.emit_op(Opcode::LdIVx(last.into()));

                    // caller-save 復帰
                    if num_to_save > 0 {
                        self.emit_op(Opcode::LdI(Addr::new(save_addr)));
                        let last_reg = UserRegister::new(num_to_save - 1);
                        self.emit_op(Opcode::LdVxI(last_reg.into()));
                    }

                    ValueLocation::InMemory {
                        addr: ret_addr,
                        struct_name: ret_struct_name,
                    }
                } else {
                    // スカラー戻り値: 新しい一時レジスタに退避して
                    // 後続の式評価で V0 が上書きされても安全にする
                    let result_temp = self.alloc_temp_register();
                    if Register::from(result_temp) != Register::V0 {
                        self.emit_op(Opcode::LdReg(result_temp.into(), Register::V0));
                    }

                    // caller-save 復帰
                    if num_to_save > 0 {
                        self.emit_op(Opcode::LdI(Addr::new(save_addr)));
                        let last_reg = UserRegister::new(num_to_save - 1);
                        self.emit_op(Opcode::LdVxI(last_reg.into()));
                    }

                    ValueLocation::InRegister(result_temp.into())
                }
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                // 条件式テンプ割り当て前に保存 (テンプをブランチで再利用可能にする)
                let saved_local_var_count = self.local_var_count;
                let saved_next_free_reg = self.next_free_reg;

                let Some(cond_reg) = self.codegen_expr(cond).register() else {
                    return ValueLocation::Void;
                };
                self.emit_op(Opcode::SneImm(cond_reg, 0x00));
                let jp_else_offset = self.emit_placeholder();

                let then_loc = self.codegen_expr(then_block);

                if let Some(else_block) = else_block {
                    let then_is_struct = matches!(
                        &then_loc,
                        ValueLocation::InMemory { .. } | ValueLocation::InRegisters { .. }
                    );
                    match &then_loc {
                        ValueLocation::InMemory { addr, struct_name } => {
                            let count = self.struct_field_count(struct_name);
                            self.emit_op(Opcode::LdI(Addr::new(*addr)));
                            let last = UserRegister::new(count as u8 - 1);
                            self.emit_op(Opcode::LdVxI(last.into()));
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..*field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        _ => {}
                    }

                    let jp_end_offset = self.emit_placeholder();

                    let else_addr = self.current_addr();
                    self.patch_at(jp_else_offset, Opcode::Jp(else_addr));

                    // else ブランチでは then のレジスタ状態をリセット
                    self.local_var_count = saved_local_var_count;
                    self.next_free_reg = saved_next_free_reg;

                    let else_loc = self.codegen_expr(else_block);

                    let else_is_struct = matches!(
                        &else_loc,
                        ValueLocation::InMemory { .. } | ValueLocation::InRegisters { .. }
                    );
                    match &else_loc {
                        ValueLocation::InMemory { addr, struct_name } => {
                            let count = self.struct_field_count(struct_name);
                            self.emit_op(Opcode::LdI(Addr::new(*addr)));
                            let last = UserRegister::new(count as u8 - 1);
                            self.emit_op(Opcode::LdVxI(last.into()));
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..*field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        _ => {}
                    }

                    let end_addr = self.current_addr();
                    self.patch_at(jp_end_offset, Opcode::Jp(end_addr));

                    match (then_loc, else_loc) {
                        _ if then_is_struct || else_is_struct => {
                            ValueLocation::InRegister(Register::V0)
                        }
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
                // ジャンプテーブル最適化: 連続整数パターンなら O(1) ジャンプ
                if self.can_use_jump_table(arms) {
                    return self.codegen_match_jump_table(scr_reg, arms);
                }
                // フォールバック: SE+JP 線形探索
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
                    self.emit_op(Opcode::SeImm(scr_reg, pattern_val));
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
            ExprKind::StructLiteral { name, fields, base } => {
                let field_count = self.struct_field_count(name);
                let struct_fields = self.struct_defs.get(name).cloned().unwrap_or_default();

                // メモリスロットを割り当て
                let struct_addr = self.alloc_mem_slot(field_count as u16);

                // base がある場合、メモリを一括コピー
                if let Some(base_expr) = base {
                    let base_loc = self.codegen_expr(base_expr);
                    if let ValueLocation::InMemory { addr: src_addr, .. } = base_loc {
                        self.emit_memcpy(src_addr, struct_addr, field_count);
                    }
                }

                // 各フィールドの値を評価してメモリに書き込み
                for (field_name, value_expr) in fields {
                    if let Some(offset) = self.struct_field_offset(name, field_name) {
                        let field_ty = struct_fields
                            .iter()
                            .find(|f| &f.name == field_name)
                            .map(|f| &f.ty);

                        if let Some(Type::UserType(sub_name)) = field_ty
                            && self.struct_defs.contains_key(sub_name)
                        {
                            // struct 型フィールド
                            let val_loc = self.codegen_expr(value_expr);
                            let sub_count = self.struct_field_count(sub_name);
                            if let ValueLocation::InMemory { addr: src_addr, .. } = val_loc {
                                self.emit_memcpy(src_addr, struct_addr + offset as u16, sub_count);
                            }
                            continue;
                        }

                        // スカラーフィールド: 評価してメモリにストア
                        if let Some(val_reg) = self.codegen_expr(value_expr).register() {
                            self.emit_store_to_memory(val_reg, struct_addr + offset as u16);
                        }
                    }
                }

                ValueLocation::InMemory {
                    addr: struct_addr,
                    struct_name: name.clone(),
                }
            }
            ExprKind::FieldAccess { expr: inner, field } => {
                // まず identifier の直接バインディングルックアップを試みる
                if let ExprKind::Ident(name) = &inner.kind
                    && let Some(binding) = self.lookup_binding(name).cloned()
                {
                    match binding {
                        LocalBinding::StructInRegisters {
                            ref struct_name,
                            base_reg,
                            field_count,
                        } => {
                            if let Some(offset) = self.struct_field_offset(struct_name, field) {
                                // フィールドが struct 型の場合、InRegisters を返す
                                if let Some(field_ty) = self.struct_field_type(struct_name, field)
                                    && let Type::UserType(ref sub_name) = field_ty
                                    && self.struct_defs.contains_key(sub_name)
                                {
                                    let sub_count = self.struct_field_count(sub_name) as u8;
                                    return ValueLocation::InRegisters {
                                        struct_name: sub_name.clone(),
                                        base_reg: UserRegister::new(
                                            base_reg.index() + offset as u8,
                                        ),
                                        field_count: sub_count,
                                    };
                                }
                                // スカラーフィールド: レジスタを直接返す (0命令!)
                                let field_reg = UserRegister::new(base_reg.index() + offset as u8);
                                return ValueLocation::InRegister(field_reg.into());
                            }
                            // フィールドが見つからない場合のフォールバック
                            let _ = field_count;
                            return ValueLocation::Void;
                        }
                        LocalBinding::StructInMemory {
                            addr,
                            ref struct_name,
                        } => {
                            if let Some(offset) = self.struct_field_offset(struct_name, field) {
                                if let Some(field_ty) = self.struct_field_type(struct_name, field)
                                    && let Type::UserType(ref sub_name) = field_ty
                                    && self.struct_defs.contains_key(sub_name)
                                {
                                    return ValueLocation::InMemory {
                                        addr: addr + offset as u16,
                                        struct_name: sub_name.clone(),
                                    };
                                }
                                let reg = self.alloc_temp_register();
                                self.emit_load_from_memory(reg.into(), addr + offset as u16);
                                return ValueLocation::InRegister(reg.into());
                            }
                            return ValueLocation::Void;
                        }
                        _ => {}
                    }
                }
                // 式の評価結果からのフィールドアクセス
                let inner_loc = self.codegen_expr(inner);
                match inner_loc {
                    ValueLocation::InRegisters {
                        ref struct_name,
                        base_reg,
                        ..
                    } => {
                        let sn = struct_name.clone();
                        if let Some(offset) = self.struct_field_offset(&sn, field) {
                            if let Some(field_ty) = self.struct_field_type(&sn, field)
                                && let Type::UserType(ref sub_name) = field_ty
                                && self.struct_defs.contains_key(sub_name)
                            {
                                let sub_count = self.struct_field_count(sub_name) as u8;
                                return ValueLocation::InRegisters {
                                    struct_name: sub_name.clone(),
                                    base_reg: UserRegister::new(base_reg.index() + offset as u8),
                                    field_count: sub_count,
                                };
                            }
                            let field_reg = UserRegister::new(base_reg.index() + offset as u8);
                            return ValueLocation::InRegister(field_reg.into());
                        }
                        ValueLocation::Void
                    }
                    ValueLocation::InMemory {
                        addr,
                        ref struct_name,
                    } => {
                        let sn = struct_name.clone();
                        if let Some(offset) = self.struct_field_offset(&sn, field) {
                            if let Some(field_ty) = self.struct_field_type(&sn, field)
                                && let Type::UserType(ref sub_name) = field_ty
                                && self.struct_defs.contains_key(sub_name)
                            {
                                return ValueLocation::InMemory {
                                    addr: addr + offset as u16,
                                    struct_name: sub_name.clone(),
                                };
                            }
                            let reg = self.alloc_temp_register();
                            self.emit_load_from_memory(reg.into(), addr + offset as u16);
                            return ValueLocation::InRegister(reg.into());
                        }
                        ValueLocation::Void
                    }
                    _ => ValueLocation::Void,
                }
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

    /// メモリ上の struct 同士の等値比較
    fn codegen_struct_equality_memory(
        &mut self,
        l_addr: u16,
        l_name: &str,
        r_addr: u16,
        _r_name: &str,
        is_eq: bool,
    ) -> ValueLocation {
        let count = self.struct_field_count(l_name);
        let res = self.alloc_temp_register();

        if count == 0 {
            self.emit_op(Opcode::LdImm(res.into(), if is_eq { 1 } else { 0 }));
            return ValueLocation::InRegister(res.into());
        }

        // res = 1 (仮に全フィールドが等しいと仮定)
        self.emit_op(Opcode::LdImm(res.into(), 1));

        for i in 0..count {
            // 左辺フィールドをロード
            let l_reg = self.alloc_temp_register();
            self.emit_load_from_memory(l_reg.into(), l_addr + i as u16);
            // 右辺フィールドをロード
            let r_reg = self.alloc_temp_register();
            self.emit_load_from_memory(r_reg.into(), r_addr + i as u16);
            // 比較: 不一致なら res = 0
            self.emit_op(Opcode::SeReg(l_reg.into(), r_reg.into()));
            self.emit_op(Opcode::LdImm(res.into(), 0));
            // テンプレジスタを解放 (res の次まで巻き戻し)
            self.next_free_reg = res.index() + 1;
        }

        if !is_eq {
            let one = self.alloc_temp_register();
            self.emit_op(Opcode::LdImm(one.into(), 1));
            self.emit_op(Opcode::Xor(res.into(), one.into()));
        }

        ValueLocation::InRegister(res.into())
    }

    /// レジスタ上の struct 同士の等値比較
    fn codegen_struct_equality_registers(
        &mut self,
        l_base: UserRegister,
        field_count: u8,
        r_base: UserRegister,
        is_eq: bool,
    ) -> ValueLocation {
        let res = self.alloc_temp_register();

        if field_count == 0 {
            self.emit_op(Opcode::LdImm(res.into(), if is_eq { 1 } else { 0 }));
            return ValueLocation::InRegister(res.into());
        }

        self.emit_op(Opcode::LdImm(res.into(), 1));

        for i in 0..field_count {
            let l_reg: Register = UserRegister::new(l_base.index() + i).into();
            let r_reg: Register = UserRegister::new(r_base.index() + i).into();
            self.emit_op(Opcode::SeReg(l_reg, r_reg));
            self.emit_op(Opcode::LdImm(res.into(), 0));
        }

        if !is_eq {
            let one = self.alloc_temp_register();
            self.emit_op(Opcode::LdImm(one.into(), 1));
            self.emit_op(Opcode::Xor(res.into(), one.into()));
        }

        ValueLocation::InRegister(res.into())
    }

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
            BuiltinFunction::RandomEnum => {
                // args[0] は Ident(enum_name)
                let enum_name = if let ExprKind::Ident(name) = &args[0].kind {
                    name.clone()
                } else {
                    return ValueLocation::Void;
                };

                // バリアント数を取得
                let count = self
                    .enum_variant_values
                    .keys()
                    .filter(|(e, _)| *e == enum_name)
                    .count() as u8;

                let res = self.alloc_temp_register();

                if count == 0 {
                    self.emit_op(Opcode::LdImm(res.into(), 0));
                    return ValueLocation::InRegister(res.into());
                }

                // mask = next_power_of_two(count) - 1
                let mask = count.next_power_of_two() - 1;

                if count.is_power_of_two() {
                    // count が 2 の冪: RND で直接生成
                    self.emit_op(Opcode::Rnd(res.into(), mask));
                } else {
                    // 拒否サンプリング: mask で生成し、count 以上なら再試行
                    let tmp = self.alloc_temp_register();
                    let loop_addr = self.current_addr();
                    self.emit_op(Opcode::Rnd(res.into(), mask));
                    // tmp = res をコピーし、tmp -= count で比較
                    self.emit_op(Opcode::LdImm(tmp.into(), count));
                    // res >= count かチェック: SUB は Vx = Vx - Vy, VF = NOT borrow
                    // res が tmp (=count) にコピーされている代わりに、
                    // SeImm で直接比較: res == count なら再試行
                    // ただし SE/SNE は即値比較のみ。count 以上の判定は SUB が必要。
                    // SUBN: tmp = count - res, VF=1 なら count >= res (つまり res <= count)
                    // VF=0 なら count < res (res > count) → 再試行
                    // res == count の場合も再試行が必要
                    // → Sub: tmp(=count) - res → tmp = count - res
                    //   VF=1: count >= res → res <= count
                    //   res < count: OK, res == count: NG, res > count: NG
                    // SUBN(tmp, res) → tmp = res - tmp = res - count, VF = 1 if res >= count
                    self.emit_op(Opcode::Subn(tmp.into(), res.into()));
                    // VF == 1 → res >= count → 再試行
                    self.emit_op(Opcode::SneImm(Register::VF, 1));
                    self.emit_op(Opcode::Jp(loop_addr));
                }

                ValueLocation::InRegister(res.into())
            }
        }
    }

    fn codegen_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                let val_loc = self.codegen_expr(value);
                // struct 型の let
                if let Type::UserType(type_name) = ty
                    && self.struct_defs.contains_key(type_name)
                {
                    match val_loc {
                        ValueLocation::InMemory { addr, .. } => {
                            self.local_bindings.insert(
                                name.clone(),
                                LocalBinding::StructInMemory {
                                    addr,
                                    struct_name: type_name.clone(),
                                },
                            );
                            self.next_free_reg = self.local_var_count;
                            return;
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            // レジスタ上の struct をそのままバインド
                            self.next_free_reg = self.local_var_count;
                            // base_reg が local_var_count 以上なら永続化のためコピー
                            let new_base = UserRegister::new(self.next_free_reg);
                            for i in 0..field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(self.next_free_reg + i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                            self.next_free_reg += field_count;
                            self.local_bindings.insert(
                                name.clone(),
                                LocalBinding::StructInRegisters {
                                    struct_name: type_name.clone(),
                                    base_reg: new_base,
                                    field_count,
                                },
                            );
                            self.local_var_count = self.next_free_reg;
                            return;
                        }
                        _ => {}
                    }
                }
                // スカラー型の let
                if let Some(val_reg) = val_loc.register() {
                    self.next_free_reg = self.local_var_count;
                    let reg = self.alloc_register();
                    if val_reg != Register::from(reg) {
                        self.emit_op(Opcode::LdReg(reg.into(), val_reg));
                    }
                    self.local_bindings
                        .insert(name.clone(), LocalBinding::Single(reg));
                    self.local_var_count = self.next_free_reg;
                    return;
                }
            }
            StmtKind::Assign { name, value } => {
                let val_loc = self.codegen_expr(value);
                // struct 変数への代入
                if let Some(LocalBinding::StructInMemory {
                    addr: target_addr, ..
                }) = self.local_bindings.get(name).cloned()
                {
                    match val_loc {
                        ValueLocation::InMemory {
                            addr: src_addr,
                            ref struct_name,
                        } => {
                            let count = self.struct_field_count(struct_name);
                            self.emit_memcpy(src_addr, target_addr, count);
                        }
                        _ => {
                            if let Some(val_reg) = val_loc.register() {
                                self.emit_store_to_memory(val_reg, target_addr);
                            }
                        }
                    }
                } else if let Some(LocalBinding::StructInRegisters {
                    base_reg: target_base,
                    field_count,
                    ..
                }) = self.local_bindings.get(name).cloned()
                {
                    match val_loc {
                        ValueLocation::InRegisters {
                            base_reg: src_base, ..
                        } => {
                            for i in 0..field_count {
                                let src = UserRegister::new(src_base.index() + i);
                                let dst = UserRegister::new(target_base.index() + i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        ValueLocation::InMemory {
                            addr,
                            ref struct_name,
                        } => {
                            let count = self.struct_field_count(struct_name);
                            for i in 0..count {
                                let dst = UserRegister::new(target_base.index() + i as u8);
                                self.emit_load_from_memory(dst.into(), addr + i as u16);
                            }
                        }
                        _ => {}
                    }
                } else if let Some(val_reg) = val_loc.register()
                    && let Some(LocalBinding::Single(target_reg)) = self.local_bindings.get(name)
                {
                    let target: Register = (*target_reg).into();
                    if val_reg != target {
                        self.emit_op(Opcode::LdReg(target, val_reg));
                    }
                } else if let Some(val_reg) = val_loc.register()
                    && self.mutable_globals.contains(name)
                {
                    // ミュータブルグローバル変数への書き込み
                    self.emit_global_write(name, val_reg);
                }
            }
            StmtKind::IndexAssign {
                array,
                index,
                value,
            } => {
                let Some(idx_reg) = self.codegen_expr(index).register() else {
                    return;
                };
                let Some(val_reg) = self.codegen_expr(value).register() else {
                    return;
                };
                // V0 に値をセット → I = base + index → FX55 で書き込み
                if val_reg == Register::V0 {
                    self.emit_ld_i_global(array);
                    self.emit_op(Opcode::AddI(idx_reg));
                    self.emit_op(Opcode::LdIVx(Register::V0));
                } else if self.v0_is_bound() {
                    // V0 退避 → 値を V0 に → 書き込み → V0 復帰
                    let tmp = self.alloc_temp_register();
                    self.emit_op(Opcode::LdReg(tmp.into(), Register::V0));
                    self.emit_op(Opcode::LdReg(Register::V0, val_reg));
                    self.emit_ld_i_global(array);
                    self.emit_op(Opcode::AddI(idx_reg));
                    self.emit_op(Opcode::LdIVx(Register::V0));
                    self.emit_op(Opcode::LdReg(Register::V0, tmp.into()));
                } else {
                    self.emit_op(Opcode::LdReg(Register::V0, val_reg));
                    self.emit_ld_i_global(array);
                    self.emit_op(Opcode::AddI(idx_reg));
                    self.emit_op(Opcode::LdIVx(Register::V0));
                }
            }
            StmtKind::Expr(expr) => {
                self.codegen_expr(expr);
            }
            StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    let loc = self.codegen_expr(e);
                    match loc {
                        ValueLocation::InMemory {
                            addr, struct_name, ..
                        } => {
                            let count = self.struct_field_count(&struct_name);
                            self.emit_op(Opcode::LdI(Addr::new(addr)));
                            let last = UserRegister::new(count as u8 - 1);
                            self.emit_op(Opcode::LdVxI(last.into()));
                        }
                        ValueLocation::InRegisters {
                            base_reg,
                            field_count,
                            ..
                        } => {
                            for i in 0..field_count {
                                let src = UserRegister::new(base_reg.index() + i);
                                let dst = UserRegister::new(i);
                                if src != dst {
                                    self.emit_op(Opcode::LdReg(dst.into(), src.into()));
                                }
                            }
                        }
                        _ => {
                            if let Some(reg) = loc.register()
                                && reg != Register::V0
                            {
                                self.emit_op(Opcode::LdReg(Register::V0, reg));
                            }
                        }
                    }
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
