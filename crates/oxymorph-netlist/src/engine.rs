//! Thin wrapper over the `lark_rs` engine.
//!
//! The engine is configured per format to mirror upstream (`decisions/0002`):
//! spectre/vams use LALR + contextual lexer; spice/hspice/xyce use
//! Earley + `ambiguity="resolve"` (dynamic lexer).

use lark_rs::{Ambiguity, Lark, LarkOptions, LexerType, ParserAlgorithm};
use lark_rs::tree::ParseTree;

use crate::error::Error;

pub struct Engine(Lark);

impl Engine {
    pub fn lalr(grammar: &str) -> Result<Self, Error> {
        let opts = LarkOptions {
            parser: ParserAlgorithm::Lalr,
            lexer: LexerType::Contextual,
            ..LarkOptions::default()
        };
        Ok(Engine(Lark::new(grammar, opts)?))
    }

    pub fn earley(grammar: &str) -> Result<Self, Error> {
        let opts = LarkOptions {
            parser: ParserAlgorithm::Earley,
            lexer: LexerType::Auto,
            ambiguity: Ambiguity::Resolve,
            ..LarkOptions::default()
        };
        Ok(Engine(Lark::new(grammar, opts)?))
    }

    pub fn parse(&self, text: &str) -> Result<ParseTree, Error> {
        Ok(self.0.parse(text)?)
    }
}
