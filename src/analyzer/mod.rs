use std::collections::HashMap;

use crate::parser::ast::*;

/// UserRegister の総数 (V0-VE)
const USER_REGISTER_COUNT: usize = 15;
/// コード生成が式評価中に消費する一時レジスタの最大数
const MAX_TEMP_REGISTERS: usize = 5;
/// 1 関数内で使えるローカル変数の上限 (パラメータ含む)
const MAX_LOCALS: usize = USER_REGISTER_COUNT - MAX_TEMP_REGISTERS;

/// 意味解析エラーの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeErrorKind {
    /// main 関数が定義されていない
    MissingMain,
    /// 未定義の変数を参照
    UndefinedVariable(String),
    /// 未定義の関数を呼び出し
    UndefinedFunction(String),
    /// 型の不一致 (context は "in let", "return type" など文脈を示す)
    TypeMismatch {
        context: &'static str,
        expected: Type,
        found: Type,
    },
    /// 二項演算の型不一致
    BinaryOpTypeMismatch { lhs: Type, rhs: Type },
    /// ユーザー定義関数の引数の数が合わない
    ArgumentCountMismatch {
        function: String,
        expected: usize,
        found: usize,
    },
    /// ユーザー定義関数の引数の型が合わない
    ArgumentTypeMismatch {
        function: String,
        expected: Type,
        found: Type,
    },
    /// 組み込み関数の引数の数が合わない
    BuiltinArgCountMismatch {
        builtin: BuiltinFunction,
        expected: usize,
        found: usize,
    },
    /// 組み込み関数の引数の型が合わない
    BuiltinArgTypeMismatch {
        builtin: BuiltinFunction,
        expected: Type,
        found: Type,
    },
    /// 論理演算子に bool 以外が渡された
    LogicalOpRequiresBool(Type),
    /// 符号反転に u8 以外が渡された
    NegationRequiresU8(Type),
    /// 論理否定に bool 以外が渡された
    LogicalNotRequiresBool(Type),
    /// if 条件が bool でない
    IfConditionNotBool(Type),
    /// if/else の型が一致しない
    IfElseBranchMismatch { then_type: Type, else_type: Type },
    /// ローカル変数が多すぎる (テンポラリレジスタ分を差し引いた上限)
    TooManyLocals { count: usize, max: usize },
    /// 配列でない型にインデックスアクセス
    CannotIndex(Type),
    /// 配列インデックスが u8 でない
    ArrayIndexNotU8(Type),
    /// 配列要素の型が統一されていない
    ArrayElementMismatch,
    /// loop の外で break
    BreakOutsideLoop,
    /// 代入の型不一致
    AssignmentTypeMismatch { expected: Type, found: Type },
    /// match の scrutinee が u8 でも enum でもない
    MatchScrutineeType(Type),
    /// match アームのパターンが不正
    MatchArmPatternNotLiteral,
    /// match アームの型が不一致
    MatchArmTypeMismatch { first: Type, found: Type },
    /// match アームが空
    MatchNoArms,
    /// 未定義の enum
    UndefinedEnum(String),
    /// 未定義の enum variant
    UndefinedEnumVariant { enum_name: String, variant: String },
    /// match の enum 網羅性不足
    NonExhaustiveMatch {
        enum_name: String,
        missing: Vec<String>,
    },
    /// 不明な型名
    UnknownType(String),
    /// random_enum の引数が enum 名でない
    RandomEnumArgNotEnum(String),
    /// 未定義の struct
    UndefinedStruct(String),
    /// 未定義のフィールド
    UndefinedField { struct_name: String, field: String },
    /// struct リテラルで必須フィールドが不足
    MissingFields {
        struct_name: String,
        missing: Vec<String>,
    },
    /// struct リテラルでフィールドが重複
    DuplicateField { struct_name: String, field: String },
    /// フィールドアクセスの対象が struct でない
    FieldAccessOnNonStruct(Type),
}

/// 意味解析エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    pub kind: AnalyzeErrorKind,
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AnalyzeErrorKind::MissingMain => write!(f, "missing 'main' function"),
            AnalyzeErrorKind::UndefinedVariable(name) => {
                write!(f, "undefined variable: '{name}'")
            }
            AnalyzeErrorKind::UndefinedFunction(name) => {
                write!(f, "undefined function: '{name}'")
            }
            AnalyzeErrorKind::TypeMismatch {
                context,
                expected,
                found,
            } => write!(f, "{context}: expected {expected:?}, found {found:?}"),
            AnalyzeErrorKind::BinaryOpTypeMismatch { lhs, rhs } => {
                write!(f, "binary op type mismatch: {lhs:?} vs {rhs:?}")
            }
            AnalyzeErrorKind::ArgumentCountMismatch {
                function,
                expected,
                found,
            } => write!(f, "'{function}' expects {expected} args, got {found}"),
            AnalyzeErrorKind::ArgumentTypeMismatch {
                function,
                expected,
                found,
            } => write!(
                f,
                "argument type mismatch in '{function}': expected {expected:?}, found {found:?}"
            ),
            AnalyzeErrorKind::BuiltinArgCountMismatch {
                builtin,
                expected,
                found,
            } => write!(
                f,
                "'{}' expects {expected} args, got {found}",
                builtin.name()
            ),
            AnalyzeErrorKind::BuiltinArgTypeMismatch {
                builtin,
                expected,
                found,
            } => write!(
                f,
                "argument type mismatch in '{}': expected {expected:?}, found {found:?}",
                builtin.name()
            ),
            AnalyzeErrorKind::LogicalOpRequiresBool(ty) => {
                write!(f, "logical op requires bool, found {ty:?}")
            }
            AnalyzeErrorKind::NegationRequiresU8(ty) => {
                write!(f, "negation requires u8, found {ty:?}")
            }
            AnalyzeErrorKind::LogicalNotRequiresBool(ty) => {
                write!(f, "logical not requires bool, found {ty:?}")
            }
            AnalyzeErrorKind::IfConditionNotBool(ty) => {
                write!(f, "if condition must be bool, found {ty:?}")
            }
            AnalyzeErrorKind::IfElseBranchMismatch {
                then_type,
                else_type,
            } => write!(f, "if/else type mismatch: {then_type:?} vs {else_type:?}"),
            AnalyzeErrorKind::TooManyLocals { count, max } => {
                write!(f, "too many local variables: {count} (max {max})")
            }
            AnalyzeErrorKind::CannotIndex(ty) => write!(f, "cannot index into {ty:?}"),
            AnalyzeErrorKind::ArrayIndexNotU8(ty) => {
                write!(f, "array index must be u8, found {ty:?}")
            }
            AnalyzeErrorKind::ArrayElementMismatch => {
                write!(f, "array elements must have the same type")
            }
            AnalyzeErrorKind::BreakOutsideLoop => write!(f, "break outside of loop"),
            AnalyzeErrorKind::AssignmentTypeMismatch { expected, found } => {
                write!(
                    f,
                    "assignment type mismatch: expected {expected:?}, found {found:?}"
                )
            }
            AnalyzeErrorKind::MatchScrutineeType(ty) => {
                write!(f, "match scrutinee must be u8 or enum, found {ty:?}")
            }
            AnalyzeErrorKind::MatchArmPatternNotLiteral => {
                write!(f, "match arm pattern must be a literal or enum variant")
            }
            AnalyzeErrorKind::MatchArmTypeMismatch { first, found } => {
                write!(f, "match arm type mismatch: {first:?} vs {found:?}")
            }
            AnalyzeErrorKind::MatchNoArms => write!(f, "match expression has no arms"),
            AnalyzeErrorKind::UndefinedEnum(name) => write!(f, "undefined enum: '{name}'"),
            AnalyzeErrorKind::UndefinedEnumVariant { enum_name, variant } => {
                write!(f, "undefined variant '{variant}' in enum '{enum_name}'")
            }
            AnalyzeErrorKind::NonExhaustiveMatch { enum_name, missing } => write!(
                f,
                "non-exhaustive match on '{enum_name}': missing {}",
                missing.join(", ")
            ),
            AnalyzeErrorKind::UnknownType(name) => write!(f, "unknown type: '{name}'"),
            AnalyzeErrorKind::RandomEnumArgNotEnum(name) => {
                write!(f, "random_enum argument must be an enum name, got '{name}'")
            }
            AnalyzeErrorKind::UndefinedStruct(name) => {
                write!(f, "undefined struct: '{name}'")
            }
            AnalyzeErrorKind::UndefinedField { struct_name, field } => {
                write!(f, "undefined field '{field}' in struct '{struct_name}'")
            }
            AnalyzeErrorKind::MissingFields {
                struct_name,
                missing,
            } => write!(
                f,
                "missing fields in struct '{struct_name}': {}",
                missing.join(", ")
            ),
            AnalyzeErrorKind::DuplicateField { struct_name, field } => {
                write!(f, "duplicate field '{field}' in struct '{struct_name}'")
            }
            AnalyzeErrorKind::FieldAccessOnNonStruct(ty) => {
                write!(f, "field access on non-struct type: {ty:?}")
            }
        }
    }
}

/// 関数シグネチャ
#[derive(Debug, Clone)]
struct FnSig {
    params: Vec<Type>,
    return_type: Type,
}

/// 意味解析器
pub struct Analyzer {
    /// グローバル変数の型
    globals: HashMap<String, Type>,
    /// ユーザー定義関数のシグネチャ
    functions: HashMap<String, FnSig>,
    /// ユーザー定義 enum (名前 → variant リスト)
    enums: HashMap<String, Vec<String>>,
    /// ユーザー定義 struct (名前 → フィールド定義リスト)
    structs: HashMap<String, Vec<StructField>>,
    /// ローカルスコープスタック
    locals: Vec<HashMap<String, Type>>,
    /// 現在の関数の戻り値型
    current_return_type: Option<Type>,
    /// loop のネスト深さ
    loop_depth: usize,
    /// エラーリスト
    errors: Vec<AnalyzeError>,
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            functions: HashMap::new(),
            enums: HashMap::new(),
            structs: HashMap::new(),
            locals: Vec::new(),
            current_return_type: None,
            loop_depth: 0,
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) -> Result<(), Vec<AnalyzeError>> {
        // Pass 1: 全トップレベル定義を登録
        for top in &program.top_levels {
            match top {
                TopLevel::LetDef { name, ty, .. } => {
                    self.globals.insert(name.clone(), ty.clone());
                }
                TopLevel::FnDef {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    self.functions.insert(
                        name.clone(),
                        FnSig {
                            params: params.iter().map(|p| p.ty.clone()).collect(),
                            return_type: return_type.clone(),
                        },
                    );
                }
                TopLevel::EnumDef { name, variants, .. } => {
                    self.enums.insert(name.clone(), variants.clone());
                }
                TopLevel::StructDef { name, fields, .. } => {
                    self.structs.insert(name.clone(), fields.clone());
                }
            }
        }

        // main 関数の存在チェック
        if !self.functions.contains_key("main") {
            self.errors.push(AnalyzeError {
                kind: AnalyzeErrorKind::MissingMain,
            });
        }

        // UserType 型の存在チェック (enum または struct)
        for top in &program.top_levels {
            let types_to_check: Vec<&Type> = match top {
                TopLevel::FnDef {
                    params,
                    return_type,
                    ..
                } => {
                    let mut ts: Vec<&Type> = params.iter().map(|p| &p.ty).collect();
                    ts.push(return_type);
                    ts
                }
                TopLevel::LetDef { ty, .. } => vec![ty],
                TopLevel::EnumDef { .. } | TopLevel::StructDef { .. } => vec![],
            };
            for ty in types_to_check {
                if let Type::UserType(name) = ty
                    && !self.enums.contains_key(name)
                    && !self.structs.contains_key(name)
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UnknownType(name.clone()),
                    });
                }
            }
        }

        // Pass 2: 各定義の本体を解析
        for top in &program.top_levels {
            match top {
                TopLevel::LetDef { ty, value, .. } => {
                    let value_ty = self.type_check_expr(value);
                    if let Some(vt) = value_ty
                        && !Self::types_compatible(ty, &vt)
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::TypeMismatch {
                                context: "type mismatch",
                                expected: ty.clone(),
                                found: vt,
                            },
                        });
                    }
                }
                TopLevel::FnDef {
                    params,
                    return_type,
                    body,
                    ..
                } => {
                    self.current_return_type = Some(return_type.clone());
                    self.locals.push(HashMap::new());

                    for param in params {
                        self.insert_local(param.name.clone(), param.ty.clone());
                    }

                    let body_ty = self.type_check_expr(body);
                    if let Some(bt) = body_ty
                        && !Self::types_compatible(return_type, &bt)
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::TypeMismatch {
                                context: "return type mismatch",
                                expected: return_type.clone(),
                                found: bt,
                            },
                        });
                    }

                    // struct 型はメモリに配置されるためレジスタを消費しない
                    if let Some(scope) = self.locals.last() {
                        let reg_count: usize = scope
                            .values()
                            .map(|ty| {
                                if let Type::UserType(name) = ty
                                    && self.structs.contains_key(name)
                                {
                                    return 0;
                                }
                                1
                            })
                            .sum();
                        if reg_count > MAX_LOCALS {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::TooManyLocals {
                                    count: reg_count,
                                    max: MAX_LOCALS,
                                },
                            });
                        }
                    }

                    self.locals.pop();
                    self.current_return_type = None;
                }
                TopLevel::EnumDef { .. } | TopLevel::StructDef { .. } => {}
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn insert_local(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<Type> {
        for scope in self.locals.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        self.globals.get(name).cloned()
    }

    fn types_compatible(expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (Type::U8, Type::U8) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Unit, Type::Unit) => true,
            (Type::Array(e1, s1), Type::Array(e2, s2)) => {
                s1 == s2 && Self::types_compatible(e1, e2)
            }
            (Type::Sprite(_), Type::Sprite(_)) => true,
            (Type::Sprite(n), Type::Array(elem, m)) => *n == *m && **elem == Type::U8,
            (Type::UserType(a), Type::UserType(b)) => a == b, // enum or struct, same name
            _ => false,
        }
    }

    fn type_check_expr(&mut self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::IntLiteral(_) => Some(Type::U8),
            ExprKind::BoolLiteral(_) => Some(Type::Bool),
            ExprKind::Ident(name) => {
                if let Some(ty) = self.lookup_var(name) {
                    Some(ty)
                } else {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UndefinedVariable(name.clone()),
                    });
                    None
                }
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.type_check_expr(lhs);
                let rhs_ty = self.type_check_expr(rhs);
                match (lhs_ty, rhs_ty) {
                    (Some(lt), Some(rt)) => {
                        if !Self::types_compatible(&lt, &rt) {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::BinaryOpTypeMismatch { lhs: lt, rhs: rt },
                            });
                            return None;
                        }
                        match op {
                            BinOp::Eq
                            | BinOp::NotEq
                            | BinOp::Lt
                            | BinOp::Gt
                            | BinOp::LtEq
                            | BinOp::GtEq => Some(Type::Bool),
                            BinOp::And | BinOp::Or => {
                                if lt != Type::Bool {
                                    self.errors.push(AnalyzeError {
                                        kind: AnalyzeErrorKind::LogicalOpRequiresBool(lt),
                                    });
                                    None
                                } else {
                                    Some(Type::Bool)
                                }
                            }
                            _ => Some(Type::U8),
                        }
                    }
                    _ => None,
                }
            }
            ExprKind::UnaryOp { op, expr: inner } => {
                let ty = self.type_check_expr(inner)?;
                match op {
                    UnaryOp::Neg => {
                        if ty != Type::U8 {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::NegationRequiresU8(ty),
                            });
                            None
                        } else {
                            Some(Type::U8)
                        }
                    }
                    UnaryOp::Not => {
                        if ty != Type::Bool {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::LogicalNotRequiresBool(ty),
                            });
                            None
                        } else {
                            Some(Type::Bool)
                        }
                    }
                }
            }
            ExprKind::BuiltinCall { builtin, args } => {
                // random_enum は引数が型名 (enum 名) なので特殊処理
                if *builtin == BuiltinFunction::RandomEnum {
                    if args.len() != 1 {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::BuiltinArgCountMismatch {
                                builtin: *builtin,
                                expected: 1,
                                found: args.len(),
                            },
                        });
                        return None;
                    }
                    if let ExprKind::Ident(name) = &args[0].kind {
                        if self.enums.contains_key(name) {
                            return Some(Type::UserType(name.clone()));
                        } else {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::RandomEnumArgNotEnum(name.clone()),
                            });
                            return None;
                        }
                    } else {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::RandomEnumArgNotEnum(
                                "<non-identifier>".to_string(),
                            ),
                        });
                        return None;
                    }
                }

                let (param_types, return_type) = builtin.signature();
                if args.len() != param_types.len() {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::BuiltinArgCountMismatch {
                            builtin: *builtin,
                            expected: param_types.len(),
                            found: args.len(),
                        },
                    });
                    return None;
                }
                for (arg, param_ty) in args.iter().zip(param_types.iter()) {
                    if let Some(arg_ty) = self.type_check_expr(arg)
                        && !Self::types_compatible(param_ty, &arg_ty)
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::BuiltinArgTypeMismatch {
                                builtin: *builtin,
                                expected: param_ty.clone(),
                                found: arg_ty,
                            },
                        });
                    }
                }
                Some(return_type)
            }
            ExprKind::Call { name, args } => {
                if let Some(sig) = self.functions.get(name).cloned() {
                    if args.len() != sig.params.len() {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::ArgumentCountMismatch {
                                function: name.clone(),
                                expected: sig.params.len(),
                                found: args.len(),
                            },
                        });
                        return None;
                    }
                    for (arg, param_ty) in args.iter().zip(sig.params.iter()) {
                        if let Some(arg_ty) = self.type_check_expr(arg)
                            && !Self::types_compatible(param_ty, &arg_ty)
                        {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::ArgumentTypeMismatch {
                                    function: name.clone(),
                                    expected: param_ty.clone(),
                                    found: arg_ty,
                                },
                            });
                        }
                    }
                    Some(sig.return_type)
                } else {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UndefinedFunction(name.clone()),
                    });
                    None
                }
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                if let Some(cond_ty) = self.type_check_expr(cond)
                    && cond_ty != Type::Bool
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::IfConditionNotBool(cond_ty),
                    });
                }
                let then_ty = self.type_check_expr(then_block);
                if let Some(else_block) = else_block {
                    let else_ty = self.type_check_expr(else_block);
                    match (then_ty, else_ty) {
                        (Some(t), Some(e)) => {
                            if !Self::types_compatible(&t, &e) {
                                self.errors.push(AnalyzeError {
                                    kind: AnalyzeErrorKind::IfElseBranchMismatch {
                                        then_type: t,
                                        else_type: e,
                                    },
                                });
                                None
                            } else {
                                Some(t)
                            }
                        }
                        _ => None,
                    }
                } else {
                    Some(Type::Unit)
                }
            }
            ExprKind::Loop { body } => {
                self.loop_depth += 1;
                self.type_check_expr(body);
                self.loop_depth -= 1;
                Some(Type::Unit)
            }
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    self.type_check_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.type_check_expr(tail)
                } else {
                    Some(Type::Unit)
                }
            }
            ExprKind::ArrayLiteral(elems) => {
                if elems.is_empty() {
                    return Some(Type::Array(Box::new(Type::U8), 0));
                }
                let first_ty = self.type_check_expr(&elems[0]);
                for elem in &elems[1..] {
                    if let (Some(ft), Some(et)) = (&first_ty, self.type_check_expr(elem))
                        && !Self::types_compatible(ft, &et)
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::ArrayElementMismatch,
                        });
                    }
                }
                first_ty.map(|t| Type::Array(Box::new(t), elems.len()))
            }
            ExprKind::Match { scrutinee, arms } => {
                let scr_ty = self.type_check_expr(scrutinee);
                if let Some(ref st) = scr_ty
                    && *st != Type::U8
                    && !matches!(st, Type::UserType(_))
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::MatchScrutineeType(st.clone()),
                    });
                }
                if arms.is_empty() {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::MatchNoArms,
                    });
                    return None;
                }
                // パターンの検証
                for arm in arms {
                    match &arm.pattern.kind {
                        ExprKind::IntLiteral(_) => {}
                        ExprKind::EnumVariant { .. } => {}
                        _ => {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::MatchArmPatternNotLiteral,
                            });
                        }
                    }
                }
                // enum 網羅性チェック
                if let Some(Type::UserType(ref enum_name)) = scr_ty
                    && let Some(all_variants) = self.enums.get(enum_name).cloned()
                {
                    let covered: Vec<String> = arms
                        .iter()
                        .filter_map(|arm| {
                            if let ExprKind::EnumVariant { variant, .. } = &arm.pattern.kind {
                                Some(variant.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let missing: Vec<String> = all_variants
                        .into_iter()
                        .filter(|v| !covered.contains(v))
                        .collect();
                    if !missing.is_empty() {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::NonExhaustiveMatch {
                                enum_name: enum_name.clone(),
                                missing,
                            },
                        });
                    }
                }
                // 全アームの body 型一致チェック
                let first_ty = self.type_check_expr(&arms[0].body)?;
                for arm in &arms[1..] {
                    if let Some(arm_ty) = self.type_check_expr(&arm.body)
                        && !Self::types_compatible(&first_ty, &arm_ty)
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::MatchArmTypeMismatch {
                                first: first_ty.clone(),
                                found: arm_ty,
                            },
                        });
                    }
                }
                Some(first_ty)
            }
            ExprKind::StructLiteral { name, fields, base } => {
                if let Some(struct_fields) = self.structs.get(name).cloned() {
                    // base がある場合、struct 型であることを確認
                    if let Some(base_expr) = base
                        && let Some(base_ty) = self.type_check_expr(base_expr)
                        && base_ty != Type::UserType(name.clone())
                    {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::TypeMismatch {
                                context: "struct update base type mismatch",
                                expected: Type::UserType(name.clone()),
                                found: base_ty,
                            },
                        });
                    }
                    // 重複フィールドチェック
                    let mut seen = Vec::new();
                    for (field_name, value) in fields {
                        if seen.contains(field_name) {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::DuplicateField {
                                    struct_name: name.clone(),
                                    field: field_name.clone(),
                                },
                            });
                        }
                        seen.push(field_name.clone());
                        // フィールド名の存在チェックと型チェック
                        if let Some(sf) = struct_fields.iter().find(|f| &f.name == field_name) {
                            if let Some(val_ty) = self.type_check_expr(value)
                                && !Self::types_compatible(&sf.ty, &val_ty)
                            {
                                self.errors.push(AnalyzeError {
                                    kind: AnalyzeErrorKind::TypeMismatch {
                                        context: "struct field type mismatch",
                                        expected: sf.ty.clone(),
                                        found: val_ty,
                                    },
                                });
                            }
                        } else {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::UndefinedField {
                                    struct_name: name.clone(),
                                    field: field_name.clone(),
                                },
                            });
                        }
                    }
                    // base なしの場合、全フィールドが指定されているかチェック
                    if base.is_none() {
                        let missing: Vec<String> = struct_fields
                            .iter()
                            .filter(|f| !seen.contains(&f.name))
                            .map(|f| f.name.clone())
                            .collect();
                        if !missing.is_empty() {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::MissingFields {
                                    struct_name: name.clone(),
                                    missing,
                                },
                            });
                        }
                    }
                    Some(Type::UserType(name.clone()))
                } else {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UndefinedStruct(name.clone()),
                    });
                    None
                }
            }
            ExprKind::FieldAccess { expr: inner, field } => {
                if let Some(ty) = self.type_check_expr(inner) {
                    if let Type::UserType(struct_name) = &ty {
                        if let Some(struct_fields) = self.structs.get(struct_name).cloned() {
                            if let Some(sf) = struct_fields.iter().find(|f| &f.name == field) {
                                Some(sf.ty.clone())
                            } else {
                                self.errors.push(AnalyzeError {
                                    kind: AnalyzeErrorKind::UndefinedField {
                                        struct_name: struct_name.clone(),
                                        field: field.clone(),
                                    },
                                });
                                None
                            }
                        } else {
                            // UserType だが struct ではない (enum)
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::FieldAccessOnNonStruct(ty),
                            });
                            None
                        }
                    } else {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::FieldAccessOnNonStruct(ty),
                        });
                        None
                    }
                } else {
                    None
                }
            }
            ExprKind::EnumVariant { enum_name, variant } => {
                if let Some(variants) = self.enums.get(enum_name) {
                    if !variants.contains(variant) {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::UndefinedEnumVariant {
                                enum_name: enum_name.clone(),
                                variant: variant.clone(),
                            },
                        });
                        None
                    } else {
                        Some(Type::UserType(enum_name.clone()))
                    }
                } else {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UndefinedEnum(enum_name.clone()),
                    });
                    None
                }
            }
            ExprKind::Index { array, index } => {
                if let Some(arr_ty) = self.type_check_expr(array) {
                    match arr_ty {
                        Type::Array(elem_ty, _) => {
                            if let Some(idx_ty) = self.type_check_expr(index)
                                && idx_ty != Type::U8
                            {
                                self.errors.push(AnalyzeError {
                                    kind: AnalyzeErrorKind::ArrayIndexNotU8(idx_ty),
                                });
                            }
                            Some(*elem_ty)
                        }
                        _ => {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::CannotIndex(arr_ty),
                            });
                            None
                        }
                    }
                } else {
                    self.type_check_expr(index);
                    None
                }
            }
        }
    }

    fn type_check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                if let Type::UserType(type_name) = ty
                    && !self.enums.contains_key(type_name)
                    && !self.structs.contains_key(type_name)
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::UnknownType(type_name.clone()),
                    });
                }
                if let Some(vt) = self.type_check_expr(value)
                    && !Self::types_compatible(ty, &vt)
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::TypeMismatch {
                            context: "type mismatch in let",
                            expected: ty.clone(),
                            found: vt,
                        },
                    });
                }
                self.insert_local(name.clone(), ty.clone());
            }
            StmtKind::Assign { name, value } => {
                let var_ty = self.lookup_var(name);
                let val_ty = self.type_check_expr(value);
                match (var_ty, val_ty) {
                    (None, _) => {
                        self.errors.push(AnalyzeError {
                            kind: AnalyzeErrorKind::UndefinedVariable(name.clone()),
                        });
                    }
                    (Some(vt), Some(et)) => {
                        if !Self::types_compatible(&vt, &et) {
                            self.errors.push(AnalyzeError {
                                kind: AnalyzeErrorKind::AssignmentTypeMismatch {
                                    expected: vt,
                                    found: et,
                                },
                            });
                        }
                    }
                    _ => {}
                }
            }
            StmtKind::Expr(expr) => {
                self.type_check_expr(expr);
            }
            StmtKind::Return(expr) => {
                let ret_ty = if let Some(e) = expr {
                    self.type_check_expr(e)
                } else {
                    Some(Type::Unit)
                };
                if let (Some(expected), Some(actual)) = (&self.current_return_type, &ret_ty)
                    && !Self::types_compatible(expected, actual)
                {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::TypeMismatch {
                            context: "return type mismatch",
                            expected: expected.clone(),
                            found: actual.clone(),
                        },
                    });
                }
            }
            StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.errors.push(AnalyzeError {
                        kind: AnalyzeErrorKind::BreakOutsideLoop,
                    });
                }
            }
        }
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}
