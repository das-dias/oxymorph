//! Tree-transform harness mirroring Lark's `Transformer`.
//!
//! `lark_rs` already reproduces Lark's `?rule`/`_rule` tree shaping, so the
//! `items` list each rule receives matches Python Lark exactly. Each format
//! implements [`TreeTransform`] with its own dynamic value type.

use lark_rs::tree::{Child, ParseTree, Token, Tree};

pub trait TreeTransform {
    type Item;

    /// Wrap a leaf token (Python's default token handling).
    fn token(&mut self, tok: &Token) -> Self::Item;

    /// Value for a `None`/maybe-placeholder child.
    fn none(&mut self) -> Self::Item;

    /// Fallback for rules with no explicit handler: rebuild a tree node.
    fn default_rule(&mut self, name: &str, items: Vec<Self::Item>) -> Self::Item;

    /// Handle a named rule. Defaults to [`Self::default_rule`].
    fn rule(&mut self, name: &str, items: Vec<Self::Item>) -> Self::Item {
        self.default_rule(name, items)
    }

    fn transform(&mut self, tree: &ParseTree) -> Self::Item {
        match tree {
            ParseTree::Tree(t) => self.tree(t),
            ParseTree::Token(t) => self.token(t),
            ParseTree::None => self.none(),
        }
    }

    fn tree(&mut self, t: &Tree) -> Self::Item {
        let items: Vec<Self::Item> = t.children.iter().map(|c| self.child(c)).collect();
        self.rule(&t.data, items)
    }

    fn child(&mut self, c: &Child) -> Self::Item {
        match c {
            Child::Tree(t) => self.tree(t),
            Child::Token(t) => self.token(t),
            Child::None => self.none(),
        }
    }
}
