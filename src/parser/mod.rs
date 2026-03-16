pub mod ast;

use crate::lexer::token::{Span, Token, TokenKind};
use ast::*;

/// Parser のエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}",
            self.span.line, self.span.column, self.message
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
                message: format!("expected {:?}, found {:?}", expected, self.peek()),
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
                message: format!("expected identifier, found {:?}", self.peek()),
                span,
            })
        }
    }

    // ---- トップレベル ----

    fn parse_top_level(&mut self) -> Result<TopLevel, ParseError> {
        match self.peek() {
            TokenKind::Fn => self.parse_fn_def(),
            TokenKind::Let => self.parse_let_def(),
            _ => Err(ParseError {
                message: format!("expected 'fn' or 'let', found {:?}", self.peek()),
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
            span,
        })
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
                    _ => Err(ParseError {
                        message: format!("unknown type: {name}"),
                        span,
                    }),
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
                message: format!("expected type, found {:?}", self.peek()),
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
                message: format!("expected integer literal, found {:?}", self.peek()),
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
                // 代入: ident = expr;
                if self.peek() == &TokenKind::Eq
                    && let ExprKind::Ident(name) = expr.kind
                {
                    self.advance(); // '='
                    let value = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    return Ok(Stmt {
                        kind: StmtKind::Assign { name, value },
                        span,
                    });
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
                // 関数呼び出し
                if self.peek() == &TokenKind::LParen {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr {
                        kind: ExprKind::Call { name, args },
                        span,
                    })
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
            TokenKind::Loop => self.parse_loop_expr(),
            _ => Err(ParseError {
                message: format!("expected expression, found {:?}", self.peek()),
                span,
            }),
        }
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
                if let ExprKind::Ident(name) = expr.kind {
                    self.advance();
                    let value = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    stmts.push(Stmt {
                        kind: StmtKind::Assign { name, value },
                        span: self.tokens[checkpoint].span,
                    });
                } else {
                    return Err(ParseError {
                        message: "invalid assignment target".to_string(),
                        span: self.current_span(),
                    });
                }
            } else if self.peek() == &TokenKind::RBrace {
                // 末尾式
                tail_expr = Some(Box::new(expr));
            } else {
                return Err(ParseError {
                    message: format!("expected ';' or '}}', found {:?}", self.peek()),
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
