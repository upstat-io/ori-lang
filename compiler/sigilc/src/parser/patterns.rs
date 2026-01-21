// Pattern parsing for Sigil
// Handles match expressions and pattern syntax

use super::Parser;
use crate::ast::*;
use crate::lexer::Token;

impl Parser {
    /// Parse named arguments in pattern syntax: .property: value
    /// Returns a list of (property_name, value) pairs
    pub(super) fn parse_named_args(&mut self) -> Result<Vec<(String, Expr)>, String> {
        let mut args = Vec::new();

        self.skip_newlines();
        while !matches!(self.current(), Some(Token::RParen)) {
            // Expect .property: value syntax
            self.expect(Token::Dot)?;

            let prop_name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected property name after '.'".to_string()),
            };

            self.expect(Token::Colon)?;

            // Parse the value expression
            let value = self.parse_expr()?;

            args.push((prop_name, value));

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Check if the current position has named arg syntax (.property:)
    pub(super) fn is_named_arg_start(&self) -> bool {
        matches!(self.current(), Some(Token::Dot)) && matches!(self.peek(1), Some(Token::Ident(_)))
    }

    pub(super) fn parse_match_expr(&mut self) -> Result<Expr, String> {
        self.expect(Token::LParen)?;
        self.skip_newlines();

        // Parse first expression - could be scrutinee or first condition
        let first_expr = self.parse_expr()?;

        // Check what follows: Comma means scrutinee, Colon means cond-style
        let is_cond_style = matches!(self.current(), Some(Token::Colon));
        let (scrutinee, mut arms): (Expr, Vec<MatchArm>) = if is_cond_style {
            // Cond-style match: match(cond: body, cond: body, default)
            // No scrutinee, just condition checks. Use Bool(true) as dummy scrutinee.
            self.advance(); // consume :
            let body = self.parse_expr()?;
            let first_arm = MatchArm {
                pattern: Pattern::Condition(first_expr),
                body,
            };
            (Expr::Bool(true), vec![first_arm])
        } else {
            // Standard match: match(scrutinee, pattern: body, ...)
            self.expect(Token::Comma)?;
            (first_expr, Vec::new())
        };

        self.skip_newlines();
        if matches!(self.current(), Some(Token::Comma)) {
            self.advance();
            self.skip_newlines();
        }

        while !matches!(self.current(), Some(Token::RParen)) {
            if is_cond_style {
                // For cond-style, parse full expressions as conditions
                let cond_expr = self.parse_expr()?;

                // Check for colon - if not present, this is the default case
                if matches!(self.current(), Some(Token::Colon)) {
                    self.advance();
                    let body = self.parse_expr()?;
                    arms.push(MatchArm {
                        pattern: Pattern::Condition(cond_expr),
                        body,
                    });
                } else {
                    // Default case: expr is the body, use wildcard pattern
                    arms.push(MatchArm {
                        pattern: Pattern::Wildcard,
                        body: cond_expr,
                    });
                }
            } else {
                // Standard match with patterns
                let pattern = self.parse_pattern()?;
                self.expect(Token::Colon)?;
                let body = self.parse_expr()?;
                arms.push(MatchArm { pattern, body });
            }

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            }
        }

        self.expect(Token::RParen)?;

        Ok(Expr::Match(Box::new(MatchExpr { scrutinee, arms })))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        match self.current() {
            Some(Token::Ident(n)) if n == "_" => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            Some(Token::Ok_) | Some(Token::Err_) | Some(Token::Some_) | Some(Token::None_) => {
                let name = match self.current() {
                    Some(Token::Ok_) => "Ok",
                    Some(Token::Err_) => "Err",
                    Some(Token::Some_) => "Some",
                    Some(Token::None_) => {
                        self.advance();
                        return Ok(Pattern::Variant {
                            name: "None".to_string(),
                            fields: Vec::new(),
                        });
                    }
                    _ => unreachable!(),
                }
                .to_string();
                self.advance();

                let fields = if matches!(self.current(), Some(Token::LBrace)) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !matches!(self.current(), Some(Token::RBrace)) {
                        let fname = match self.current() {
                            Some(Token::Ident(n)) => {
                                let n = n.clone();
                                self.advance();
                                n
                            }
                            _ => return Err("Expected field name in pattern".to_string()),
                        };
                        fields.push((fname.clone(), Pattern::Binding(fname)));

                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RBrace)?;
                    fields
                } else {
                    Vec::new()
                };

                Ok(Pattern::Variant { name, fields })
            }
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();

                // Check if it's a variant pattern with fields
                if matches!(self.current(), Some(Token::LBrace)) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !matches!(self.current(), Some(Token::RBrace)) {
                        let fname = match self.current() {
                            Some(Token::Ident(f)) => {
                                let f = f.clone();
                                self.advance();
                                f
                            }
                            _ => return Err("Expected field name".to_string()),
                        };
                        fields.push((fname.clone(), Pattern::Binding(fname)));

                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RBrace)?;
                    return Ok(Pattern::Variant { name: n, fields });
                }

                Ok(Pattern::Binding(n))
            }
            Some(Token::Int(_))
            | Some(Token::String(_))
            | Some(Token::True)
            | Some(Token::False) => {
                let expr = self.parse_primary_expr()?;
                Ok(Pattern::Literal(expr))
            }
            _ => {
                // Try parsing as a condition
                let expr = self.parse_expr()?;
                Ok(Pattern::Condition(expr))
            }
        }
    }

    /// Disambiguate between pattern keyword (fold, map, filter, etc.) and function call.
    /// Called after we've already consumed the identifier and know it's followed by '('.
    /// Supports both positional args and named property syntax (.property: value).
    pub(super) fn parse_pattern_or_call_from_ident(
        &mut self,
        keyword: &str,
    ) -> Result<Expr, String> {
        // Parse args (we know current token is '(')
        self.advance(); // consume '('
        self.skip_newlines();

        // Check if we have named property syntax
        if self.is_named_arg_start() {
            return self.parse_pattern_with_named_args(keyword);
        }

        // Positional args
        let args = self.parse_args()?;
        self.expect(Token::RParen)?;

        // Check if arg count matches pattern signature
        let is_pattern = match keyword {
            "fold" => args.len() == 3,    // fold(collection, init, op)
            "map" => args.len() == 2,     // map(collection, transform)
            "filter" => args.len() == 2,  // filter(collection, predicate)
            "collect" => args.len() == 2, // collect(range, transform)
            "recurse" => args.len() == 3 || args.len() == 4, // recurse(cond, base, step) or recurse(cond, base, step, memo)
            _ => false,
        };

        if is_pattern {
            match keyword {
                "fold" => Ok(Expr::Pattern(PatternExpr::Fold {
                    collection: Box::new(args[0].clone()),
                    init: Box::new(args[1].clone()),
                    op: Box::new(args[2].clone()),
                })),
                "map" => Ok(Expr::Pattern(PatternExpr::Map {
                    collection: Box::new(args[0].clone()),
                    transform: Box::new(args[1].clone()),
                })),
                "filter" => Ok(Expr::Pattern(PatternExpr::Filter {
                    collection: Box::new(args[0].clone()),
                    predicate: Box::new(args[1].clone()),
                })),
                "collect" => Ok(Expr::Pattern(PatternExpr::Collect {
                    range: Box::new(args[0].clone()),
                    transform: Box::new(args[1].clone()),
                })),
                "recurse" => {
                    // Check if fourth arg is `true` for memoization
                    let memo = if args.len() >= 4 {
                        matches!(args[3], Expr::Bool(true))
                    } else {
                        false
                    };
                    Ok(Expr::Pattern(PatternExpr::Recurse {
                        condition: Box::new(args[0].clone()),
                        base_value: Box::new(args[1].clone()),
                        step: Box::new(args[2].clone()),
                        memo,
                        parallel_threshold: 0, // positional syntax doesn't support parallel yet
                    }))
                }
                _ => Ok(Expr::Call {
                    func: Box::new(Expr::Ident(keyword.to_string())),
                    args,
                }),
            }
        } else {
            // Not a pattern - treat as function call
            Ok(Expr::Call {
                func: Box::new(Expr::Ident(keyword.to_string())),
                args,
            })
        }
    }

    /// Parse a pattern expression with named property syntax
    /// e.g., recurse(.cond: n <= 1, .base: 1, .step: n * self(n-1), .memo: true)
    fn parse_pattern_with_named_args(&mut self, keyword: &str) -> Result<Expr, String> {
        let named_args = self.parse_named_args()?;
        self.expect(Token::RParen)?;

        // Convert named args to a hashmap for easy lookup
        let mut props: std::collections::HashMap<String, Expr> = named_args.into_iter().collect();

        match keyword {
            "recurse" => {
                // Required: cond, base, step
                // Optional: memo (default false), parallel (default false)
                let condition = props
                    .remove("cond")
                    .ok_or_else(|| "recurse pattern requires .cond: property".to_string())?;
                let base_value = props
                    .remove("base")
                    .ok_or_else(|| "recurse pattern requires .base: property".to_string())?;
                let step = props
                    .remove("step")
                    .ok_or_else(|| "recurse pattern requires .step: property".to_string())?;
                let memo = props
                    .remove("memo")
                    .map(|e| matches!(e, Expr::Bool(true)))
                    .unwrap_or(false);
                // .parallel: N means parallelize when n > N
                // .parallel: 0 means always parallelize
                // absent means no parallelization
                let parallel_threshold = props
                    .remove("parallel")
                    .map(|e| match e {
                        Expr::Int(n) => n,
                        _ => i64::MAX, // invalid value = no parallelization
                    })
                    .unwrap_or(i64::MAX);

                Ok(Expr::Pattern(PatternExpr::Recurse {
                    condition: Box::new(condition),
                    base_value: Box::new(base_value),
                    step: Box::new(step),
                    memo,
                    parallel_threshold,
                }))
            }
            "fold" => {
                // Required: over, init, op
                let collection = props
                    .remove("over")
                    .ok_or_else(|| "fold pattern requires .over: property".to_string())?;
                let init = props
                    .remove("init")
                    .ok_or_else(|| "fold pattern requires .init: property".to_string())?;
                let op = props
                    .remove("op")
                    .ok_or_else(|| "fold pattern requires .op: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Fold {
                    collection: Box::new(collection),
                    init: Box::new(init),
                    op: Box::new(op),
                }))
            }
            "map" => {
                // Required: over, transform
                let collection = props
                    .remove("over")
                    .ok_or_else(|| "map pattern requires .over: property".to_string())?;
                let transform = props
                    .remove("transform")
                    .ok_or_else(|| "map pattern requires .transform: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Map {
                    collection: Box::new(collection),
                    transform: Box::new(transform),
                }))
            }
            "filter" => {
                // Required: over, predicate
                let collection = props
                    .remove("over")
                    .ok_or_else(|| "filter pattern requires .over: property".to_string())?;
                let predicate = props
                    .remove("predicate")
                    .ok_or_else(|| "filter pattern requires .predicate: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Filter {
                    collection: Box::new(collection),
                    predicate: Box::new(predicate),
                }))
            }
            "collect" => {
                // Required: range, transform
                let range = props
                    .remove("range")
                    .ok_or_else(|| "collect pattern requires .range: property".to_string())?;
                let transform = props
                    .remove("transform")
                    .ok_or_else(|| "collect pattern requires .transform: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Collect {
                    range: Box::new(range),
                    transform: Box::new(transform),
                }))
            }
            "parallel" => {
                // Optional: timeout, on_error
                // All other properties become branches
                let timeout = props.remove("timeout").map(|e| Box::new(e));
                let on_error = props
                    .remove("on_error")
                    .map(|e| {
                        if let Expr::Ident(s) = &e {
                            if s == "collect_all" {
                                OnError::CollectAll
                            } else {
                                OnError::FailFast
                            }
                        } else {
                            OnError::FailFast
                        }
                    })
                    .unwrap_or(OnError::FailFast);

                // Remaining props are the branches
                let branches: Vec<(String, Expr)> = props.into_iter().collect();

                if branches.is_empty() {
                    return Err("parallel pattern requires at least one branch".to_string());
                }

                Ok(Expr::Pattern(PatternExpr::Parallel {
                    branches,
                    timeout,
                    on_error,
                }))
            }
            _ => Err(format!(
                "Unknown pattern keyword with named args: {}",
                keyword
            )),
        }
    }
}
