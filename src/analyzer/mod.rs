use std::collections::HashMap;

use crate::parser::ast::*;

/// 意味解析エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    pub message: String,
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
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
    /// 関数シグネチャ
    functions: HashMap<String, FnSig>,
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
        let mut analyzer = Self {
            globals: HashMap::new(),
            functions: HashMap::new(),
            locals: Vec::new(),
            current_return_type: None,
            loop_depth: 0,
            errors: Vec::new(),
        };
        analyzer.register_builtins();
        analyzer
    }

    fn register_builtins(&mut self) {
        // clear()
        self.functions.insert(
            "clear".into(),
            FnSig {
                params: vec![],
                return_type: Type::Unit,
            },
        );
        // draw(sprite, x, y) -> bool
        // sprite は任意の sprite 型として、ここでは特殊扱い
        self.functions.insert(
            "draw".into(),
            FnSig {
                params: vec![Type::Sprite(0), Type::U8, Type::U8],
                return_type: Type::Bool,
            },
        );
        // wait_key() -> u8
        self.functions.insert(
            "wait_key".into(),
            FnSig {
                params: vec![],
                return_type: Type::U8,
            },
        );
        // is_key_pressed(k) -> bool
        self.functions.insert(
            "is_key_pressed".into(),
            FnSig {
                params: vec![Type::U8],
                return_type: Type::Bool,
            },
        );
        // delay() -> u8
        self.functions.insert(
            "delay".into(),
            FnSig {
                params: vec![],
                return_type: Type::U8,
            },
        );
        // set_delay(v: u8)
        self.functions.insert(
            "set_delay".into(),
            FnSig {
                params: vec![Type::U8],
                return_type: Type::Unit,
            },
        );
        // set_sound(v: u8)
        self.functions.insert(
            "set_sound".into(),
            FnSig {
                params: vec![Type::U8],
                return_type: Type::Unit,
            },
        );
        // random(mask: u8) -> u8
        self.functions.insert(
            "random".into(),
            FnSig {
                params: vec![Type::U8],
                return_type: Type::U8,
            },
        );
        // bcd(v: u8)
        self.functions.insert(
            "bcd".into(),
            FnSig {
                params: vec![Type::U8],
                return_type: Type::Unit,
            },
        );
        // draw_digit(v: u8, x, y)
        self.functions.insert(
            "draw_digit".into(),
            FnSig {
                params: vec![Type::U8, Type::U8, Type::U8],
                return_type: Type::Unit,
            },
        );
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
            }
        }

        // main 関数の存在チェック
        if !self.functions.contains_key("main") {
            self.errors.push(AnalyzeError {
                message: "missing 'main' function".into(),
            });
        }

        // Pass 2: 各定義の本体を解析
        for top in &program.top_levels {
            match top {
                TopLevel::LetDef { ty, value, .. } => {
                    let value_ty = self.check_expr(value);
                    if let Some(vt) = value_ty
                        && !Self::types_compatible(ty, &vt)
                    {
                        self.errors.push(AnalyzeError {
                            message: format!("type mismatch: expected {:?}, found {:?}", ty, vt),
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

                    // 引数をローカルスコープに追加
                    for param in params {
                        self.insert_local(param.name.clone(), param.ty.clone());
                    }

                    let body_ty = self.check_expr(body);
                    if let Some(bt) = body_ty
                        && !Self::types_compatible(return_type, &bt)
                    {
                        self.errors.push(AnalyzeError {
                            message: format!(
                                "return type mismatch: expected {:?}, found {:?}",
                                return_type, bt
                            ),
                        });
                    }

                    // ローカル変数数チェック (引数含めて最大15個)
                    if let Some(scope) = self.locals.last()
                        && scope.len() > 15
                    {
                        self.errors.push(AnalyzeError {
                            message: format!(
                                "too many local variables: {} (max 15, V0-VE)",
                                scope.len()
                            ),
                        });
                    }

                    self.locals.pop();
                    self.current_return_type = None;
                }
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
        // ローカルスコープを後ろから探索
        for scope in self.locals.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        // グローバル
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
            // sprite は u8 配列リテラルで初期化可能
            (Type::Sprite(n), Type::Array(elem, m)) => *n == *m && **elem == Type::U8,
            _ => false,
        }
    }

    /// 式の型チェック。型を返す (エラー時は None)
    fn check_expr(&mut self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::IntLiteral(_) => Some(Type::U8),
            ExprKind::BoolLiteral(_) => Some(Type::Bool),
            ExprKind::Ident(name) => {
                if let Some(ty) = self.lookup_var(name) {
                    Some(ty)
                } else {
                    self.errors.push(AnalyzeError {
                        message: format!("undefined variable: '{name}'"),
                    });
                    None
                }
            }
            ExprKind::BinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.check_expr(lhs);
                let rhs_ty = self.check_expr(rhs);
                match (lhs_ty, rhs_ty) {
                    (Some(lt), Some(rt)) => {
                        if !Self::types_compatible(&lt, &rt) {
                            self.errors.push(AnalyzeError {
                                message: format!("binary op type mismatch: {:?} vs {:?}", lt, rt),
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
                                        message: format!(
                                            "logical op requires bool, found {:?}",
                                            lt
                                        ),
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
                let ty = self.check_expr(inner)?;
                match op {
                    UnaryOp::Neg => {
                        if ty != Type::U8 {
                            self.errors.push(AnalyzeError {
                                message: format!("negation requires u8, found {:?}", ty),
                            });
                            None
                        } else {
                            Some(Type::U8)
                        }
                    }
                    UnaryOp::Not => {
                        if ty != Type::Bool {
                            self.errors.push(AnalyzeError {
                                message: format!("logical not requires bool, found {:?}", ty),
                            });
                            None
                        } else {
                            Some(Type::Bool)
                        }
                    }
                }
            }
            ExprKind::Call { name, args } => {
                if let Some(sig) = self.functions.get(name).cloned() {
                    if args.len() != sig.params.len() {
                        self.errors.push(AnalyzeError {
                            message: format!(
                                "'{}' expects {} args, got {}",
                                name,
                                sig.params.len(),
                                args.len()
                            ),
                        });
                        return None;
                    }
                    for (arg, param_ty) in args.iter().zip(sig.params.iter()) {
                        if let Some(arg_ty) = self.check_expr(arg)
                            && !Self::types_compatible(param_ty, &arg_ty)
                        {
                            self.errors.push(AnalyzeError {
                                message: format!(
                                    "argument type mismatch in '{}': expected {:?}, found {:?}",
                                    name, param_ty, arg_ty
                                ),
                            });
                        }
                    }
                    Some(sig.return_type)
                } else {
                    self.errors.push(AnalyzeError {
                        message: format!("undefined function: '{name}'"),
                    });
                    None
                }
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                if let Some(cond_ty) = self.check_expr(cond)
                    && cond_ty != Type::Bool
                {
                    self.errors.push(AnalyzeError {
                        message: format!("if condition must be bool, found {:?}", cond_ty),
                    });
                }
                let then_ty = self.check_expr(then_block);
                if let Some(else_block) = else_block {
                    let else_ty = self.check_expr(else_block);
                    match (then_ty, else_ty) {
                        (Some(t), Some(e)) => {
                            if !Self::types_compatible(&t, &e) {
                                self.errors.push(AnalyzeError {
                                    message: format!("if/else type mismatch: {:?} vs {:?}", t, e),
                                });
                                None
                            } else {
                                Some(t)
                            }
                        }
                        _ => None,
                    }
                } else {
                    // if without else → Unit
                    Some(Type::Unit)
                }
            }
            ExprKind::Loop { body } => {
                self.loop_depth += 1;
                self.check_expr(body);
                self.loop_depth -= 1;
                Some(Type::Unit)
            }
            ExprKind::Block { stmts, expr } => {
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.check_expr(tail)
                } else {
                    Some(Type::Unit)
                }
            }
            ExprKind::ArrayLiteral(elems) => {
                if elems.is_empty() {
                    return Some(Type::Array(Box::new(Type::U8), 0));
                }
                let first_ty = self.check_expr(&elems[0]);
                for elem in &elems[1..] {
                    if let (Some(ft), Some(et)) = (&first_ty, self.check_expr(elem))
                        && !Self::types_compatible(ft, &et)
                    {
                        self.errors.push(AnalyzeError {
                            message: "array elements must have the same type".into(),
                        });
                    }
                }
                first_ty.map(|t| Type::Array(Box::new(t), elems.len()))
            }
            ExprKind::Index { array, index } => {
                if let Some(arr_ty) = self.check_expr(array) {
                    match arr_ty {
                        Type::Array(elem_ty, _) => {
                            if let Some(idx_ty) = self.check_expr(index)
                                && idx_ty != Type::U8
                            {
                                self.errors.push(AnalyzeError {
                                    message: format!("array index must be u8, found {:?}", idx_ty),
                                });
                            }
                            Some(*elem_ty)
                        }
                        _ => {
                            self.errors.push(AnalyzeError {
                                message: format!("cannot index into {:?}", arr_ty),
                            });
                            None
                        }
                    }
                } else {
                    self.check_expr(index);
                    None
                }
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                if let Some(vt) = self.check_expr(value)
                    && !Self::types_compatible(ty, &vt)
                {
                    self.errors.push(AnalyzeError {
                        message: format!("type mismatch in let: expected {:?}, found {:?}", ty, vt),
                    });
                }
                self.insert_local(name.clone(), ty.clone());
            }
            StmtKind::Assign { name, value } => {
                let var_ty = self.lookup_var(name);
                let val_ty = self.check_expr(value);
                match (var_ty, val_ty) {
                    (None, _) => {
                        self.errors.push(AnalyzeError {
                            message: format!("undefined variable: '{name}'"),
                        });
                    }
                    (Some(vt), Some(et)) => {
                        if !Self::types_compatible(&vt, &et) {
                            self.errors.push(AnalyzeError {
                                message: format!(
                                    "assignment type mismatch: expected {:?}, found {:?}",
                                    vt, et
                                ),
                            });
                        }
                    }
                    _ => {}
                }
            }
            StmtKind::Expr(expr) => {
                self.check_expr(expr);
            }
            StmtKind::Return(expr) => {
                let ret_ty = if let Some(e) = expr {
                    self.check_expr(e)
                } else {
                    Some(Type::Unit)
                };
                if let (Some(expected), Some(actual)) = (&self.current_return_type, &ret_ty)
                    && !Self::types_compatible(expected, actual)
                {
                    self.errors.push(AnalyzeError {
                        message: format!(
                            "return type mismatch: expected {:?}, found {:?}",
                            expected, actual
                        ),
                    });
                }
            }
            StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.errors.push(AnalyzeError {
                        message: "break outside of loop".into(),
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
