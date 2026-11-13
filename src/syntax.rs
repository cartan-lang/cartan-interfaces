//! Parsing: source text to a syntax tree with locations.

/// A parsed document or expression with source locations.
///
/// Gap: this is a placeholder for the tree the syntax component already
/// publishes. The parts of it Graph relies on are not listed yet.
pub struct SyntaxTree {
    _private: (),
}

/// Parse text. Problems are diagnostics carried in the tree, not a failure.
pub trait Parse {
    fn parse_document(&self, source: &str) -> SyntaxTree;
}
