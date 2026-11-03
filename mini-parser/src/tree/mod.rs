//! Lossless syntax trees, in two layers, as in rust-analyzer's `rowan`.
//!
//! - The **green** tree is the data: each node has a kind, a length and
//!   its children; each token has a kind and its text. It knows no
//!   positions and no parents, so it's immutable and can be shared: two
//!   identical subtrees could be one, and an edit could rebuild only the
//!   path from the change to the root.
//! - The **red** tree is a view over the green one, built on demand as
//!   you walk it: each `SyntaxNode` knows its parent and its absolute
//!   position in the text.
//!
//! Every byte of the source is in exactly one token, whitespace and
//! comments included: the tree's text is the source text.

mod builder;

pub(crate) use builder::build_tree;

use std::fmt::{self, Write as _};
use std::rc::Rc;

use crate::SyntaxKind;
use crate::text_range::TextRange;

// The green tree ---------------------------------------------------------

/// A token in the green tree: a kind and its text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GreenToken {
    kind: SyntaxKind,
    text: String,
}

/// A node in the green tree: a kind and children. Its length is cached.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GreenNode {
    kind: SyntaxKind,
    text_len: u32,
    children: Vec<GreenElement>,
}

/// A child of a green node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GreenElement {
    Node(Rc<GreenNode>),
    Token(Rc<GreenToken>),
}

impl GreenToken {
    pub fn new(kind: SyntaxKind, text: &str) -> GreenToken {
        GreenToken {
            kind,
            text: text.to_string(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl GreenNode {
    pub fn new(kind: SyntaxKind, children: Vec<GreenElement>) -> GreenNode {
        let text_len = children.iter().map(GreenElement::text_len).sum();
        GreenNode {
            kind,
            text_len,
            children,
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// The length of the node's text, in bytes.
    pub fn text_len(&self) -> u32 {
        self.text_len
    }

    pub fn children(&self) -> &[GreenElement] {
        &self.children
    }
}

impl GreenElement {
    pub fn kind(&self) -> SyntaxKind {
        match self {
            GreenElement::Node(node) => node.kind(),
            GreenElement::Token(token) => token.kind(),
        }
    }

    pub fn text_len(&self) -> u32 {
        match self {
            GreenElement::Node(node) => node.text_len(),
            GreenElement::Token(token) => token.text().len() as u32,
        }
    }
}

/// Builds a green tree from the top down: start a node, add its
/// children, finish it.
#[derive(Default)]
pub struct GreenNodeBuilder {
    /// The nodes started but not finished, innermost last, with the
    /// children each has so far.
    parents: Vec<(SyntaxKind, Vec<GreenElement>)>,
    /// The outermost node, once it's finished.
    root: Option<Rc<GreenNode>>,
}

impl GreenNodeBuilder {
    pub fn start_node(&mut self, kind: SyntaxKind) {
        todo!("Part 6a: start_node")
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        todo!("Part 6a: token")
    }

    pub fn finish_node(&mut self) {
        todo!("Part 6a: finish_node")
    }

    /// The finished tree's root.
    pub fn finish(self) -> Rc<GreenNode> {
        todo!("Part 6a: finish")
    }
}

// The red tree -----------------------------------------------------------

/// A node of the syntax tree, with its position and its parent. Cheap
/// to clone: it's a pointer.
#[derive(Clone)]
pub struct SyntaxNode(Rc<NodeData>);

struct NodeData {
    green: Rc<GreenNode>,
    parent: Option<SyntaxNode>,
    /// Where the node starts in the text.
    offset: u32,
}

/// A token of the syntax tree, with its position and its parent.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SyntaxToken {
    green: Rc<GreenToken>,
    parent: SyntaxNode,
    offset: u32,
}

/// A child in the syntax tree: a node or a token.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxToken),
}

impl SyntaxNode {
    pub fn new_root(green: Rc<GreenNode>) -> SyntaxNode {
        SyntaxNode(Rc::new(NodeData {
            green,
            parent: None,
            offset: 0,
        }))
    }

    pub fn kind(&self) -> SyntaxKind {
        self.0.green.kind()
    }

    pub fn text_range(&self) -> TextRange {
        TextRange::new(self.0.offset, self.0.offset + self.0.green.text_len())
    }

    pub fn parent(&self) -> Option<SyntaxNode> {
        self.0.parent.clone()
    }

    pub fn green(&self) -> &GreenNode {
        &self.0.green
    }

    /// The children, nodes and tokens, in order.
    pub fn children_with_tokens(&self) -> impl Iterator<Item = SyntaxElement> + use<> {
        std::iter::from_fn(|| todo!("Part 6b: children_with_tokens"))
    }

    /// The child nodes, in order. Tokens are skipped.
    pub fn children(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        self.children_with_tokens().filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node),
            SyntaxElement::Token(_) => None,
        })
    }

    /// This node and all the nodes below it, parents before children.
    pub fn descendants(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        std::iter::from_fn(|| todo!("Part 6b: descendants"))
    }

    /// The node's text: all its tokens, trivia included.
    pub fn text(&self) -> String {
        let mut text = String::new();
        self.write_text(&mut text);
        text
    }

    fn write_text(&self, out: &mut String) {
        todo!("Part 6b: write_text")
    }

    /// The tree in rust-analyzer's format: one line per node or token,
    /// with its kind and range, indented by depth. Tokens show their text.
    pub fn debug_dump(&self) -> String {
        let mut out = String::new();
        self.dump(0, &mut out);
        out
    }

    fn dump(&self, depth: usize, out: &mut String) {
        todo!("Part 6c: dump")
    }
}

impl PartialEq for SyntaxNode {
    /// The same node: the same green node at the same place.
    fn eq(&self, other: &SyntaxNode) -> bool {
        Rc::ptr_eq(&self.0.green, &other.0.green) && self.0.offset == other.0.offset
    }
}

impl Eq for SyntaxNode {}

impl std::hash::Hash for SyntaxNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0.green).hash(state);
        self.0.offset.hash(state);
    }
}

impl fmt::Debug for SyntaxNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}@{}", self.kind(), self.text_range())
    }
}

impl SyntaxToken {
    pub fn kind(&self) -> SyntaxKind {
        self.green.kind()
    }

    pub fn text(&self) -> &str {
        self.green.text()
    }

    pub fn text_range(&self) -> TextRange {
        TextRange::new(self.offset, self.offset + self.green.text().len() as u32)
    }

    pub fn parent(&self) -> SyntaxNode {
        self.parent.clone()
    }
}

impl fmt::Debug for SyntaxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}@{} {:?}",
            self.kind(),
            self.text_range(),
            self.text()
        )
    }
}

impl SyntaxElement {
    pub fn kind(&self) -> SyntaxKind {
        match self {
            SyntaxElement::Node(node) => node.kind(),
            SyntaxElement::Token(token) => token.kind(),
        }
    }

    pub fn text_range(&self) -> TextRange {
        match self {
            SyntaxElement::Node(node) => node.text_range(),
            SyntaxElement::Token(token) => token.text_range(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyntaxKind::*;
    use expect_test::expect;

    /// `f(x)` by hand: CALL_EXPR(PATH_EXPR(NAME_REF("f")), ARG_LIST("(", …, ")")).
    fn call() -> Rc<GreenNode> {
        let mut builder = GreenNodeBuilder::default();
        builder.start_node(CALL_EXPR);
        builder.start_node(PATH_EXPR);
        builder.start_node(NAME_REF);
        builder.token(LOWER_IDENT, "f");
        builder.finish_node();
        builder.finish_node();
        builder.start_node(ARG_LIST);
        builder.token(L_PAREN, "(");
        builder.token(WHITESPACE, " ");
        builder.start_node(PATH_EXPR);
        builder.start_node(NAME_REF);
        builder.token(LOWER_IDENT, "x");
        builder.finish_node();
        builder.finish_node();
        builder.token(R_PAREN, ")");
        builder.finish_node();
        builder.finish_node();
        builder.finish()
    }

    #[test]
    fn green_nodes_know_their_length() {
        let green = call();
        assert_eq!(green.kind(), CALL_EXPR);
        assert_eq!(green.text_len(), 5);
        assert_eq!(green.children().len(), 2);
        assert_eq!(green.children()[1].text_len(), 4);
    }

    #[test]
    #[should_panic(expected = "unfinished")]
    fn finish_with_a_node_still_open_panics() {
        let mut builder = GreenNodeBuilder::default();
        builder.start_node(SOURCE_FILE);
        builder.start_node(FN);
        builder.finish_node();
        builder.finish();
    }

    #[test]
    fn children_know_where_they_are() {
        let root = SyntaxNode::new_root(call());
        let children: Vec<SyntaxNode> = root.children().collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind(), PATH_EXPR);
        assert_eq!(children[0].text_range(), TextRange::new(0, 1));
        assert_eq!(children[1].kind(), ARG_LIST);
        assert_eq!(children[1].text_range(), TextRange::new(1, 5));
        assert_eq!(children[1].parent(), Some(root.clone()));
        let tokens: Vec<String> = children[1]
            .children_with_tokens()
            .map(|child| format!("{:?}@{}", child.kind(), child.text_range()))
            .collect();
        assert_eq!(
            tokens,
            [
                "L_PAREN@1..2",
                "WHITESPACE@2..3",
                "PATH_EXPR@3..4",
                "R_PAREN@4..5"
            ]
        );
    }

    #[test]
    fn descendants_come_parents_first() {
        let root = SyntaxNode::new_root(call());
        let kinds: Vec<SyntaxKind> = root.descendants().map(|node| node.kind()).collect();
        assert_eq!(
            kinds,
            [
                CALL_EXPR, PATH_EXPR, NAME_REF, ARG_LIST, PATH_EXPR, NAME_REF
            ]
        );
    }

    #[test]
    fn the_text_of_a_node_is_all_its_tokens() {
        let root = SyntaxNode::new_root(call());
        assert_eq!(root.text(), "f( x)");
        assert_eq!(root.children().nth(1).unwrap().text(), "( x)");
    }

    #[test]
    fn debug_dump() {
        let root = SyntaxNode::new_root(call());
        expect![[r#"
            CALL_EXPR@0..5
              PATH_EXPR@0..1
                NAME_REF@0..1
                  LOWER_IDENT@0..1 "f"
              ARG_LIST@1..5
                L_PAREN@1..2 "("
                WHITESPACE@2..3 " "
                PATH_EXPR@3..4
                  NAME_REF@3..4
                    LOWER_IDENT@3..4 "x"
                R_PAREN@4..5 ")"
        "#]]
        .assert_eq(&root.debug_dump());
    }
}
