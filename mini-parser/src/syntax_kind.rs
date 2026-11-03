//! Every kind of token and of node in a Maxim syntax tree.
//!
//! One enum for both, as in rust-analyzer: a tree is made of nodes and
//! tokens, and each of them has a `SyntaxKind`. Tokens come from the
//! lexer (`FN_KW`, `L_PAREN`, `WHITESPACE`…), nodes from the parser
//! (`FN`, `CALL_EXPR`…). The names are SCREAMING_CASE because they read
//! like the grammar's terminals, and rust-analyzer does the same.

/// The kind of a token or a node.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SyntaxKind {
    // Special kinds.
    /// What an abandoned marker leaves behind; never in a finished tree.
    TOMBSTONE,
    /// The end of the input. The parser sees it; the tree never has it.
    EOF,
    /// A token the lexer couldn't make sense of, or a node that wraps
    /// tokens the parser couldn't make sense of.
    ERROR,

    // Trivia: kept in the tree, ignored by the parser.
    WHITESPACE,
    COMMENT,

    // Names and literals.
    LOWER_IDENT,
    UPPER_IDENT,
    INT_NUMBER,
    FLOAT_NUMBER,
    STRING,
    /// `"text${`: an interpolated string, up to its first hole.
    STRING_START,
    /// `}text${`: from the end of a hole to the start of the next.
    STRING_MIDDLE,
    /// `}text"`: from the end of the last hole to the closing quote.
    STRING_END,

    // Keywords.
    MODULE_KW,
    IMPORT_KW,
    TYPE_KW,
    FN_KW,
    CAST_KW,
    COMPONENT_KW,
    LET_KW,
    MATCH_KW,
    IF_KW,
    ELSE_KW,

    // Punctuation.
    L_PAREN,
    R_PAREN,
    L_CURLY,
    R_CURLY,
    L_BRACK,
    R_BRACK,
    L_ANGLE,
    R_ANGLE,
    COMMA,
    COLON,
    COLON2,
    DOT,
    DOT2,
    EQ,
    EQ2,
    NEQ,
    LTEQ,
    GTEQ,
    PIPE,
    PIPE2,
    PIPE_GT,
    AMP2,
    BANG,
    THIN_ARROW,
    FAT_ARROW,
    QUESTION,
    PLUS,
    PLUS2,
    MINUS,
    STAR,
    STAR2,
    SLASH,
    TILDE_SLASH,
    PERCENT,
    UNDERSCORE,

    // Nodes: the file and names.
    SOURCE_FILE,
    MODULE_HEADER,
    /// A name being defined: a function, a parameter, a type…
    NAME,
    /// A name being used: a variable, a type, a field, a label…
    NAME_REF,

    // Nodes: declarations.
    IMPORT,
    TYPE_DECL,
    TYPE_PARAM_LIST,
    VARIANT_LIST,
    VARIANT,
    TUPLE_FIELD_LIST,
    RECORD_FIELD_LIST,
    RECORD_FIELD,
    FN,
    CAST,
    COMPONENT,
    LET_DECL,
    PARAM_LIST,
    PARAM,
    RET_TYPE,

    // Nodes: types.
    NAMED_TYPE,
    TYPE_ARG_LIST,
    FN_TYPE,
    PARAM_TYPE_LIST,
    PARAM_TYPE,

    // Nodes: patterns.
    WILDCARD_PAT,
    IDENT_PAT,
    LITERAL_PAT,
    CTOR_PAT,
    LIST_PAT,
    REST_PAT,

    // Nodes: expressions and statements.
    LITERAL,
    PATH_EXPR,
    HOLE_EXPR,
    PAREN_EXPR,
    LIST_EXPR,
    SPREAD,
    BLOCK_EXPR,
    LET_STMT,
    EXPR_STMT,
    LAMBDA_EXPR,
    IF_EXPR,
    MATCH_EXPR,
    MATCH_ARM_LIST,
    MATCH_ARM,
    MATCH_GUARD,
    INTERPOLATED_STRING,
    BIN_EXPR,
    PREFIX_EXPR,
    CALL_EXPR,
    ARG_LIST,
    LABELED_ARG,
    FIELD_EXPR,
    TRY_EXPR,
    CAST_EXPR,
}

use SyntaxKind::*;

impl SyntaxKind {
    /// Whitespace and comments: in the tree, but invisible to the parser.
    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | COMMENT)
    }

    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            MODULE_KW
                | IMPORT_KW
                | TYPE_KW
                | FN_KW
                | CAST_KW
                | COMPONENT_KW
                | LET_KW
                | MATCH_KW
                | IF_KW
                | ELSE_KW
        )
    }

    /// The keyword spelled `text`, if it is one.
    pub fn from_keyword(text: &str) -> Option<SyntaxKind> {
        let kind = match text {
            "module" => MODULE_KW,
            "import" => IMPORT_KW,
            "type" => TYPE_KW,
            "fn" => FN_KW,
            "cast" => CAST_KW,
            "component" => COMPONENT_KW,
            "let" => LET_KW,
            "match" => MATCH_KW,
            "if" => IF_KW,
            "else" => ELSE_KW,
            _ => return None,
        };
        Some(kind)
    }

    /// The punctuation spelled `text`, if it is one.
    pub fn from_punct(text: &str) -> Option<SyntaxKind> {
        let kind = match text {
            "(" => L_PAREN,
            ")" => R_PAREN,
            "{" => L_CURLY,
            "}" => R_CURLY,
            "[" => L_BRACK,
            "]" => R_BRACK,
            "<" => L_ANGLE,
            ">" => R_ANGLE,
            "," => COMMA,
            ":" => COLON,
            "::" => COLON2,
            "." => DOT,
            ".." => DOT2,
            "=" => EQ,
            "==" => EQ2,
            "!=" => NEQ,
            "<=" => LTEQ,
            ">=" => GTEQ,
            "|" => PIPE,
            "||" => PIPE2,
            "|>" => PIPE_GT,
            "&&" => AMP2,
            "!" => BANG,
            "->" => THIN_ARROW,
            "=>" => FAT_ARROW,
            "?" => QUESTION,
            "+" => PLUS,
            "++" => PLUS2,
            "-" => MINUS,
            "*" => STAR,
            "**" => STAR2,
            "/" => SLASH,
            "~/" => TILDE_SLASH,
            "%" => PERCENT,
            _ => return None,
        };
        Some(kind)
    }

    /// How an error message names this kind: "expected `)`", "expected
    /// a lowercase name".
    pub fn describe(self) -> &'static str {
        match self {
            EOF => "the end of the file",
            LOWER_IDENT => "a lowercase name",
            UPPER_IDENT => "a capitalized name",
            INT_NUMBER | FLOAT_NUMBER => "a number",
            STRING | STRING_START => "a string",
            STRING_MIDDLE | STRING_END => "`}`",
            MODULE_KW => "`module`",
            IMPORT_KW => "`import`",
            TYPE_KW => "`type`",
            FN_KW => "`fn`",
            CAST_KW => "`cast`",
            COMPONENT_KW => "`component`",
            LET_KW => "`let`",
            MATCH_KW => "`match`",
            IF_KW => "`if`",
            ELSE_KW => "`else`",
            L_PAREN => "`(`",
            R_PAREN => "`)`",
            L_CURLY => "`{`",
            R_CURLY => "`}`",
            L_BRACK => "`[`",
            R_BRACK => "`]`",
            L_ANGLE => "`<`",
            R_ANGLE => "`>`",
            COMMA => "`,`",
            COLON => "`:`",
            COLON2 => "`::`",
            DOT => "`.`",
            DOT2 => "`..`",
            EQ => "`=`",
            EQ2 => "`==`",
            NEQ => "`!=`",
            LTEQ => "`<=`",
            GTEQ => "`>=`",
            PIPE => "`|`",
            PIPE2 => "`||`",
            PIPE_GT => "`|>`",
            AMP2 => "`&&`",
            BANG => "`!`",
            THIN_ARROW => "`->`",
            FAT_ARROW => "`=>`",
            QUESTION => "`?`",
            PLUS => "`+`",
            PLUS2 => "`++`",
            MINUS => "`-`",
            STAR => "`*`",
            STAR2 => "`**`",
            SLASH => "`/`",
            TILDE_SLASH => "`~/`",
            PERCENT => "`%`",
            UNDERSCORE => "`_`",
            _ => "something else",
        }
    }
}

/// The kind of a keyword or a punctuation token, by its spelling:
/// `T![fn]`, `T![->]`, `T!['(']`. Borrowed from rust-analyzer: grammar
/// code reads like the grammar.
#[macro_export]
macro_rules! T {
    [module] => { $crate::SyntaxKind::MODULE_KW };
    [import] => { $crate::SyntaxKind::IMPORT_KW };
    [type] => { $crate::SyntaxKind::TYPE_KW };
    [fn] => { $crate::SyntaxKind::FN_KW };
    [cast] => { $crate::SyntaxKind::CAST_KW };
    [component] => { $crate::SyntaxKind::COMPONENT_KW };
    [let] => { $crate::SyntaxKind::LET_KW };
    [match] => { $crate::SyntaxKind::MATCH_KW };
    [if] => { $crate::SyntaxKind::IF_KW };
    [else] => { $crate::SyntaxKind::ELSE_KW };
    ['('] => { $crate::SyntaxKind::L_PAREN };
    [')'] => { $crate::SyntaxKind::R_PAREN };
    ['{'] => { $crate::SyntaxKind::L_CURLY };
    ['}'] => { $crate::SyntaxKind::R_CURLY };
    ['['] => { $crate::SyntaxKind::L_BRACK };
    [']'] => { $crate::SyntaxKind::R_BRACK };
    [<] => { $crate::SyntaxKind::L_ANGLE };
    [>] => { $crate::SyntaxKind::R_ANGLE };
    [,] => { $crate::SyntaxKind::COMMA };
    [:] => { $crate::SyntaxKind::COLON };
    [::] => { $crate::SyntaxKind::COLON2 };
    [.] => { $crate::SyntaxKind::DOT };
    [..] => { $crate::SyntaxKind::DOT2 };
    [=] => { $crate::SyntaxKind::EQ };
    [==] => { $crate::SyntaxKind::EQ2 };
    [!=] => { $crate::SyntaxKind::NEQ };
    [<=] => { $crate::SyntaxKind::LTEQ };
    [>=] => { $crate::SyntaxKind::GTEQ };
    [|] => { $crate::SyntaxKind::PIPE };
    [||] => { $crate::SyntaxKind::PIPE2 };
    [|>] => { $crate::SyntaxKind::PIPE_GT };
    [&&] => { $crate::SyntaxKind::AMP2 };
    [!] => { $crate::SyntaxKind::BANG };
    [->] => { $crate::SyntaxKind::THIN_ARROW };
    [=>] => { $crate::SyntaxKind::FAT_ARROW };
    [?] => { $crate::SyntaxKind::QUESTION };
    [+] => { $crate::SyntaxKind::PLUS };
    [++] => { $crate::SyntaxKind::PLUS2 };
    [-] => { $crate::SyntaxKind::MINUS };
    [*] => { $crate::SyntaxKind::STAR };
    [**] => { $crate::SyntaxKind::STAR2 };
    [/] => { $crate::SyntaxKind::SLASH };
    [~/] => { $crate::SyntaxKind::TILDE_SLASH };
    [%] => { $crate::SyntaxKind::PERCENT };
    [_] => { $crate::SyntaxKind::UNDERSCORE };
}
