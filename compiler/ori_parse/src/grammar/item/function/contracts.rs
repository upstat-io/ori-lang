//! Function contract parsing (`pre`/`post` conditions).

use crate::{ParseError, Parser};
use ori_ir::{Name, PostContract, PreContract, TokenKind};

use crate::context::ParseContext;

/// Whether a contextual contract identifier is `pre` or `post`.
enum ContractKind {
    Pre,
    Post,
}

impl Parser<'_> {
    /// Parse zero or more function contracts.
    ///
    /// Grammar: `{ contract }` where `contract = pre_contract | post_contract`
    ///
    /// `pre` and `post` are contextual identifiers — only recognized in this
    /// specific position between guard clause and `=` in function declarations.
    pub(super) fn parse_contracts(
        &mut self,
    ) -> Result<(Vec<PreContract>, Vec<PostContract>), ParseError> {
        let mut pre_contracts = Vec::new();
        let mut post_contracts = Vec::new();

        loop {
            self.cursor.skip_newlines();
            match self.check_contract_ident() {
                Some(ContractKind::Pre) => {
                    let contract = self
                        .parse_pre_contract()
                        .map_err(|e| e.with_context("while parsing a pre-condition contract"))?;
                    pre_contracts.push(contract);
                }
                Some(ContractKind::Post) => {
                    let contract = self
                        .parse_post_contract()
                        .map_err(|e| e.with_context("while parsing a post-condition contract"))?;
                    post_contracts.push(contract);
                }
                None => break,
            }
        }

        Ok((pre_contracts, post_contracts))
    }

    /// Check if the current token is a contextual contract keyword (`pre` or `post`).
    fn check_contract_ident(&self) -> Option<ContractKind> {
        if let TokenKind::Ident(name) = self.cursor.current_kind() {
            match self.cursor.interner().lookup(*name) {
                "pre" => Some(ContractKind::Pre),
                "post" => Some(ContractKind::Post),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Parse a pre-condition contract.
    ///
    /// Grammar: `pre_contract = "pre" "(" check_expr ")" .`
    /// where `check_expr = expression [ "|" string_literal ] .`
    fn parse_pre_contract(&mut self) -> Result<PreContract, ParseError> {
        let start = self.cursor.current_span();
        self.cursor.advance(); // consume `pre` identifier

        self.cursor.expect(&TokenKind::LParen)?;

        // Parse condition with PIPE_IS_SEPARATOR so `|` stops the expression
        let condition = self
            .with_context(ParseContext::PIPE_IS_SEPARATOR, Self::parse_expr)
            .into_result()?;

        // Optional message: `| "message"`
        let message = self.parse_contract_message()?;

        let end = self.cursor.current_span();
        self.cursor.expect(&TokenKind::RParen)?;

        Ok(PreContract {
            condition,
            message,
            span: start.merge(end),
        })
    }

    /// Parse a post-condition contract.
    ///
    /// Grammar: `post_contract = "post" "(" postcheck_expr ")" .`
    /// where `postcheck_expr = lambda_params "->" check_expr .`
    fn parse_post_contract(&mut self) -> Result<PostContract, ParseError> {
        let start = self.cursor.current_span();
        self.cursor.advance(); // consume `post` identifier

        self.cursor.expect(&TokenKind::LParen)?;

        // Parse lambda parameters: `r ->` or `(a, b) ->`
        let params = self.parse_post_contract_params()?;

        self.cursor.expect(&TokenKind::Arrow)?;

        // Parse condition with PIPE_IS_SEPARATOR so `|` stops the expression
        let condition = self
            .with_context(ParseContext::PIPE_IS_SEPARATOR, Self::parse_expr)
            .into_result()?;

        // Optional message: `| "message"`
        let message = self.parse_contract_message()?;

        let end = self.cursor.current_span();
        self.cursor.expect(&TokenKind::RParen)?;

        Ok(PostContract {
            params,
            condition,
            message,
            span: start.merge(end),
        })
    }

    /// Parse post-condition lambda parameters: `r` or `(a, b)`.
    fn parse_post_contract_params(&mut self) -> Result<Vec<Name>, ParseError> {
        if self.cursor.check(&TokenKind::LParen) {
            // Tuple destructure: `(a, b)`
            self.cursor.advance(); // consume `(`
            let mut params = Vec::new();

            let first = self.cursor.expect_ident()?;
            params.push(first);

            while self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance(); // consume `,`
                if self.cursor.check(&TokenKind::RParen) {
                    break; // trailing comma
                }
                let name = self.cursor.expect_ident()?;
                params.push(name);
            }

            self.cursor.expect(&TokenKind::RParen)?;
            Ok(params)
        } else {
            // Single parameter: `r`
            let name = self.cursor.expect_ident()?;
            Ok(vec![name])
        }
    }

    /// Parse optional contract message: `| "message"`.
    fn parse_contract_message(&mut self) -> Result<Option<Name>, ParseError> {
        if self.cursor.check(&TokenKind::Pipe) {
            self.cursor.advance(); // consume `|`
            match self.cursor.current_kind().clone() {
                TokenKind::String(name) => {
                    self.cursor.advance(); // consume string
                    Ok(Some(name))
                }
                _ => Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    format!(
                        "expected string literal for contract message, found {}",
                        self.cursor.current_kind().display_name()
                    ),
                    self.cursor.current_span(),
                )),
            }
        } else {
            Ok(None)
        }
    }
}
