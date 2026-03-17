pub mod ast;

use crate::lexer::token::{Span, Token, TokenKind};
use ast::*;

/// パースエラーの種類
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// 期待したトークンと異なるトークンが出現
    UnexpectedToken { expected: String, found: TokenKind },
    /// 型名として認識できない識別子
    UnknownType(String),
    /// 代入のターゲットが識別子でない
    InvalidAssignmentTarget,
}

/// パースエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
}

impl ParseError {
    pub fn message(&self) -> String {
        match &self.kind {
            ParseErrorKind::UnexpectedToken { expected, found } => {
                format!("expected {expected}, found {found:?}")
            }
            ParseErrorKind::UnknownType(name) => format!("unknown type: {name}"),
            ParseErrorKind::InvalidAssignmentTarget => "invalid assignment target".to_string(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}",
            self.span.line,
            self.span.column,
            self.message()
        )
    }
}

/// 再帰下降パーサー
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut top_levels = Vec::new();
        while !self.is_at_end() {
            top_levels.push(self.parse_top_level()?);
        }
        Ok(Program { top_levels })
    }

    // ---- ユーティリティ ----

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        if self.pos + offset < self.tokens.len() {
            &self.tokens[self.pos + offset].kind
        } else {
            &TokenKind::Eof
        }
    }

    fn current_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn is_at_end(&self) -> bool {
        self.peek() == &TokenKind::Eof
    }

    fn advance(&mut self) -> &Token {
        let token = &self.tokens[self.pos];
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<Span, ParseError> {
        let span = self.current_span();
        if self.peek() == expected {
            self.advance();
            Ok(span)
        } else {
            Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: format!("{:?}", expected),
                    found: self.peek().clone(),
                },
                span,
            })
        }
    }

    fn expect_ident(&mut self) -> Result<(String, Span), ParseError> {
        let span = self.current_span();
        if let TokenKind::Ident(name) = self.peek().clone() {
            self.advance();
            Ok((name, span))
        } else {
            Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: "identifier".to_string(),
                    found: self.peek().clone(),
                },
                span,
            })
        }
    }

    // ---- トップレベル ----

    fn parse_top_level(&mut self) -> Result<TopLevel, ParseError> {
        match self.peek() {
            TokenKind::Fn => self.parse_fn_def(),
            TokenKind::Let => self.parse_let_def(),
            TokenKind::Enum => self.parse_enum_def(),
            TokenKind::Struct => self.parse_struct_def(),
            _ => Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: "'fn', 'let', 'enum', or 'struct'".to_string(),
                    found: self.peek().clone(),
                },
                span: self.current_span(),
            }),
        }
    }

    fn parse_fn_def(&mut self) -> Result<TopLevel, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Arrow)?;
        let return_type = self.parse_type()?;
        let body = self.parse_block_expr()?;
        Ok(TopLevel::FnDef {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.peek() == &TokenKind::RParen {
            return Ok(params);
        }
        loop {
            let (name, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            if self.peek() == &TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    fn parse_let_def(&mut self) -> Result<TopLevel, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Let)?;
        let mutable = *self.peek() == TokenKind::Mut;
        if mutable {
            self.advance();
        }
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(TopLevel::LetDef {
            name,
            ty,
            value,
            mutable,
            span,
        })
    }

    fn parse_enum_def(&mut self) -> Result<TopLevel, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Enum)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while self.peek() != &TokenKind::RBrace {
            let (variant, _) = self.expect_ident()?;
            variants.push(variant);
            if self.peek() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(TopLevel::EnumDef {
            name,
            variants,
            span,
        })
    }

    fn parse_struct_def(&mut self) -> Result<TopLevel, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Struct)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while self.peek() != &TokenKind::RBrace {
            let (field_name, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            fields.push(StructField {
                name: field_name,
                ty,
            });
            if self.peek() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(TopLevel::StructDef { name, fields, span })
    }

    // ---- 型 ----

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                match name.as_str() {
                    "u8" => Ok(Type::U8),
                    "bool" => Ok(Type::Bool),
                    "sprite" => {
                        self.expect(&TokenKind::LParen)?;
                        let size = self.parse_int_literal()?;
                        self.expect(&TokenKind::RParen)?;
                        Ok(Type::Sprite(size as usize))
                    }
                    _ => Ok(Type::UserType(name)),
                }
            }
            TokenKind::LParen => {
                self.advance();
                self.expect(&TokenKind::RParen)?;
                Ok(Type::Unit)
            }
            TokenKind::LBracket => {
                self.advance();
                let elem_type = self.parse_type()?;
                self.expect(&TokenKind::Semicolon)?;
                let size = self.parse_int_literal()?;
                self.expect(&TokenKind::RBracket)?;
                Ok(Type::Array(Box::new(elem_type), size as usize))
            }
            _ => Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: "type".to_string(),
                    found: self.peek().clone(),
                },
                span,
            }),
        }
    }

    fn parse_int_literal(&mut self) -> Result<u64, ParseError> {
        let span = self.current_span();
        if let TokenKind::IntLiteral(v) = self.peek() {
            let v = *v;
            self.advance();
            Ok(v)
        } else {
            Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: "integer literal".to_string(),
                    found: self.peek().clone(),
                },
                span,
            })
        }
    }

    // ---- 文 ----

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Let => {
                self.advance();
                let (name, _) = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.expect(&TokenKind::Eq)?;
                let value = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt {
                    kind: StmtKind::Let { name, ty, value },
                    span,
                })
            }
            TokenKind::Return => {
                self.advance();
                if self.peek() == &TokenKind::Semicolon {
                    self.advance();
                    Ok(Stmt {
                        kind: StmtKind::Return(None),
                        span,
                    })
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    Ok(Stmt {
                        kind: StmtKind::Return(Some(expr)),
                        span,
                    })
                }
            }
            TokenKind::Break => {
                self.advance();
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt {
                    kind: StmtKind::Break,
                    span,
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                if self.peek() == &TokenKind::Eq {
                    if matches!(&expr.kind, ExprKind::Ident(_)) {
                        let ExprKind::Ident(name) = expr.kind else {
                            unreachable!()
                        };
                        self.advance(); // '='
                        let value = self.parse_expr()?;
                        self.expect(&TokenKind::Semicolon)?;
                        return Ok(Stmt {
                            kind: StmtKind::Assign { name, value },
                            span,
                        });
                    }
                    if matches!(
                        &expr.kind,
                        ExprKind::Index { array, .. } if matches!(&array.kind, ExprKind::Ident(_))
                    ) {
                        let ExprKind::Index { array, index } = expr.kind else {
                            unreachable!()
                        };
                        let ExprKind::Ident(array_name) = array.kind else {
                            unreachable!()
                        };
                        self.advance(); // '='
                        let value = self.parse_expr()?;
                        self.expect(&TokenKind::Semicolon)?;
                        return Ok(Stmt {
                            kind: StmtKind::IndexAssign {
                                array: array_name,
                                index: *index,
                                value,
                            },
                            span,
                        });
                    }
                }
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt {
                    kind: StmtKind::Expr(expr),
                    span,
                })
            }
        }
    }

    // ---- 式 (Pratt parsing) ----

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;

        loop {
            // パイプ演算子: 最低優先度、左結合
            if self.peek() == &TokenKind::Pipe && min_bp == 0 {
                self.advance();
                lhs = self.parse_pipe_rhs(lhs)?;
                continue;
            }

            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::NotEq => BinOp::NotEq,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::GtEq => BinOp::GtEq,
                TokenKind::AndAnd => BinOp::And,
                TokenKind::OrOr => BinOp::Or,
                _ => break,
            };

            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }

            let span = lhs.span;
            self.advance();
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expr {
                kind: ExprKind::BinaryOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }

        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Neg,
                        expr: Box::new(expr),
                    },
                    span,
                })
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        expr: Box::new(expr),
                    },
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.peek() == &TokenKind::LBracket {
                let span = expr.span;
                self.advance();
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RBracket)?;
                expr = Expr {
                    kind: ExprKind::Index {
                        array: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                };
            } else if self.peek() == &TokenKind::Dot {
                let span = expr.span;
                self.advance();
                let (field, _) = self.expect_ident()?;
                expr = Expr {
                    kind: ExprKind::FieldAccess {
                        expr: Box::new(expr),
                        field,
                    },
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::IntLiteral(v) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::IntLiteral(v),
                    span,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::BoolLiteral(true),
                    span,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::BoolLiteral(false),
                    span,
                })
            }
            TokenKind::Ident(name) => {
                self.advance();
                // enum variant: Name::Variant
                if self.peek() == &TokenKind::ColonColon {
                    self.advance();
                    let (variant, _) = self.expect_ident()?;
                    return Ok(Expr {
                        kind: ExprKind::EnumVariant {
                            enum_name: name,
                            variant,
                        },
                        span,
                    });
                }
                // struct リテラル: Name { field: value, ... }
                // 先読みで { Ident : ... } or { .. } or { } パターンを確認
                if self.peek() == &TokenKind::LBrace {
                    let is_struct_literal = match self.peek_at(1) {
                        TokenKind::DotDot => true, // Name { ..base }
                        TokenKind::Ident(_) => {
                            // Name { field: ... } なら struct リテラル
                            matches!(self.peek_at(2), TokenKind::Colon)
                        }
                        _ => false,
                    };
                    if is_struct_literal {
                        return self.parse_struct_literal(name, span);
                    }
                }
                // 関数呼び出し
                if self.peek() == &TokenKind::LParen {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect(&TokenKind::RParen)?;
                    // 組み込み関数ならBuiltinCallに解決
                    let kind = if let Some(builtin) = BuiltinFunction::from_name(&name) {
                        ExprKind::BuiltinCall { builtin, args }
                    } else {
                        ExprKind::Call { name, args }
                    };
                    Ok(Expr { kind, span })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Ident(name),
                        span,
                    })
                }
            }
            TokenKind::LParen => {
                self.advance();
                if self.peek() == &TokenKind::RParen {
                    self.advance();
                    // Unit リテラル: ()
                    return Ok(Expr {
                        kind: ExprKind::IntLiteral(0), // Unit を 0 として扱う
                        span,
                    });
                }
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                if self.peek() != &TokenKind::RBracket {
                    loop {
                        elems.push(self.parse_expr()?);
                        if self.peek() == &TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(Expr {
                    kind: ExprKind::ArrayLiteral(elems),
                    span,
                })
            }
            TokenKind::LBrace => Ok(self.parse_block_expr()?),
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Loop => self.parse_loop_expr(),
            _ => Err(ParseError {
                kind: ParseErrorKind::UnexpectedToken {
                    expected: "expression".to_string(),
                    found: self.peek().clone(),
                },
                span,
            }),
        }
    }

    fn parse_struct_literal(&mut self, name: String, span: Span) -> Result<Expr, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut base = None;
        while self.peek() != &TokenKind::RBrace {
            // struct update 構文: ..base
            if self.peek() == &TokenKind::DotDot {
                self.advance();
                base = Some(Box::new(self.parse_expr()?));
                if self.peek() == &TokenKind::Comma {
                    self.advance();
                }
                continue;
            }
            let (field_name, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            fields.push((field_name, value));
            if self.peek() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expr {
            kind: ExprKind::StructLiteral { name, fields, base },
            span,
        })
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.peek() == &TokenKind::RParen {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if self.peek() == &TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(args)
    }

    fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail_expr = None;

        while self.peek() != &TokenKind::RBrace {
            // 末尾式かもしれないので、まず文として解析を試みる
            let checkpoint = self.pos;

            // let, return, break は明確に文
            match self.peek() {
                TokenKind::Let | TokenKind::Return | TokenKind::Break => {
                    stmts.push(self.parse_stmt()?);
                    continue;
                }
                _ => {}
            }

            let expr = self.parse_expr()?;

            if self.peek() == &TokenKind::Semicolon {
                self.advance();
                // 代入かもしれない
                stmts.push(Stmt {
                    kind: StmtKind::Expr(expr),
                    span: self.tokens[checkpoint].span,
                });
            } else if self.peek() == &TokenKind::Eq {
                // 代入文
                if matches!(&expr.kind, ExprKind::Ident(_)) {
                    let ExprKind::Ident(name) = expr.kind else {
                        unreachable!()
                    };
                    self.advance();
                    let value = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    stmts.push(Stmt {
                        kind: StmtKind::Assign { name, value },
                        span: self.tokens[checkpoint].span,
                    });
                } else if matches!(
                    &expr.kind,
                    ExprKind::Index { array, .. } if matches!(&array.kind, ExprKind::Ident(_))
                ) {
                    let ExprKind::Index { array, index } = expr.kind else {
                        unreachable!()
                    };
                    let ExprKind::Ident(array_name) = array.kind else {
                        unreachable!()
                    };
                    self.advance();
                    let value = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    stmts.push(Stmt {
                        kind: StmtKind::IndexAssign {
                            array: array_name,
                            index: *index,
                            value,
                        },
                        span: self.tokens[checkpoint].span,
                    });
                } else {
                    return Err(ParseError {
                        kind: ParseErrorKind::InvalidAssignmentTarget,
                        span: self.current_span(),
                    });
                }
            } else if self.peek() == &TokenKind::RBrace {
                // 末尾式
                tail_expr = Some(Box::new(expr));
            } else {
                return Err(ParseError {
                    kind: ParseErrorKind::UnexpectedToken {
                        expected: "';' or '}'".to_string(),
                        found: self.peek().clone(),
                    },
                    span: self.current_span(),
                });
            }
        }

        self.expect(&TokenKind::RBrace)?;
        Ok(Expr {
            kind: ExprKind::Block {
                stmts,
                expr: tail_expr,
            },
            span,
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::If)?;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block_expr()?;
        let else_block = if self.peek() == &TokenKind::Else {
            self.advance();
            if self.peek() == &TokenKind::If {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                Some(Box::new(self.parse_block_expr()?))
            }
        } else {
            None
        };
        Ok(Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_block: Box::new(then_block),
                else_block,
            },
            span,
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(&TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while self.peek() != &TokenKind::RBrace {
            let pattern = self.parse_primary()?;
            self.expect(&TokenKind::FatArrow)?;
            let body = self.parse_expr()?;
            arms.push(MatchArm { pattern, body });
            if self.peek() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expr {
            kind: ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span,
        })
    }

    fn parse_pipe_rhs(&mut self, piped: Expr) -> Result<Expr, ParseError> {
        let span = piped.span;
        let (name, _) = self.expect_ident()?;
        let mut args = vec![piped];
        if self.peek() == &TokenKind::LParen {
            self.advance();
            if self.peek() != &TokenKind::RParen {
                loop {
                    args.push(self.parse_expr()?);
                    if self.peek() == &TokenKind::Comma {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&TokenKind::RParen)?;
        }
        let kind = if let Some(builtin) = BuiltinFunction::from_name(&name) {
            ExprKind::BuiltinCall { builtin, args }
        } else {
            ExprKind::Call { name, args }
        };
        Ok(Expr { kind, span })
    }

    fn parse_loop_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::Loop)?;
        let body = self.parse_block_expr()?;
        Ok(Expr {
            kind: ExprKind::Loop {
                body: Box::new(body),
            },
            span,
        })
    }
}

/// 二項演算子の結合力 (左結合力, 右結合力)
fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (1, 2),
        BinOp::And => (3, 4),
        BinOp::Eq | BinOp::NotEq => (5, 6),
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => (7, 8),
        BinOp::Add | BinOp::Sub => (9, 10),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (11, 12),
    }
}
