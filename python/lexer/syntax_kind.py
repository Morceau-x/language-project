from dataclasses import dataclass
from enum import Enum


class SpecialKind(Enum):
    START_OF_FILE = "SOF"
    END_OF_FILE = "EOF"
    ERROR = "ERROR"

class KeywordKind(Enum):
    MODULE = ("MODULE", "module")
    IMPORT = ("IMPORT", "import")
    AS = ("AS", "as")
    TYPE = ("TYPE", "type")
    FN = ("FN", "fn")
    CAST = ("CAST", "cast")
    LET = ("LET", "let")
    MATCH = ("MATCH", "match")
    IF = ("IF", "if")
    ELSE = ("ELSE", "else")

class IdentKind(Enum):
    LOWER = "LOWER"
    UPPER = "UPPER"
    INT = "INT"
    FLOAT = "FLOAT"
    BIN = "BINARY"
    OCT = "OCTAL"
    DEC = "DECIMAL"
    HEX = "HEXADECIMAL"
    EXP = "EXPONENT"
    STRING_START = "STRING_START" # From " to ${
    STRING_MIDDLE = "STRING_MIDDLE" # From } to ${
    STRING_END = "STRING_END" # From } to "
    STRING = "STRING" # From " to "
    LINE_COMMENT = "LINE_COMMENT"
    BLOCK_COMMENT = "BLOCK_COMMENT"

@dataclass(frozen=True, slots=True)
class Keyword:
    kind: KeywordKind

class SyntaxKind(Enum):
    # Special kinds.
    TOMBSTONE = ("TOMBSTONE", None)
    EOF = ("EOF", None)
    ERROR = ("ERROR", None)

    # Trivia: kept in the tree, ignored by the parser.
    WHITESPACE = ("WHITESPACE", None)
    COMMENT = ("COMMENT", None)

    # Names and literals.
    LOWER_IDENT = ("LOWER_IDENT", None)
    UPPER_IDENT = ("UPPER_IDENT", None)
    INT_NUMBER = ("INT_NUMBER", None)
    FLOAT_NUMBER = ("FLOAT_NUMBER", None)
    STRING = ("STRING", None)
    STRING_START = ("STRING_START", None)
    STRING_MIDDLE = ("STRING_MIDDLE", None)
    STRING_END = ("STRING_END", None)

    # Keywords.


    # Punctuation.
    L_PAREN = "L_PAREN"
    R_PAREN = "R_PAREN"
    L_CURLY = "L_CURLY"
    R_CURLY = "R_CURLY"
    L_BRACK = "L_BRACK"
    R_BRACK = "R_BRACK"
    L_ANGLE = "L_ANGLE"
    R_ANGLE = "R_ANGLE"
    COMMA = "COMMA"
    COLON = "COLON"
    COLON2 = "COLON2"
    DOT = "DOT"
    DOT2 = "DOT2"
    EQ = "EQ"
    EQ2 = "EQ2"
    NEQ = "NEQ"
    LTEQ = "LTEQ"
    GTEQ = "GTEQ"
    PIPE = "PIPE"
    PIPE2 = "PIPE2"
    PIPE_GT = "PIPE_GT"
    AMP2 = "AMP2"
    BANG = "BANG"
    THIN_ARROW = "THIN_ARROW"
    FAT_ARROW = "FAT_ARROW"
    QUESTION = "QUESTION"
    PLUS = "PLUS"
    PLUS2 = "PLUS2"
    MINUS = "MINUS"
    STAR = "STAR"
    STAR2 = "STAR2"
    SLASH = "SLASH"
    TILDE_SLASH = "TILDE_SLASH"
    PERCENT = "PERCENT"
    UNDERSCORE = "UNDERSCORE"

    # Nodes: the file and names.
    SOURCE_FILE = "SOURCE_FILE"
    MODULE_HEADER = "MODULE_HEADER"
    NAME = "NAME"
    NAME_REF = "NAME_REF"

    # Nodes: declarations.
    IMPORT = "IMPORT"
    TYPE_DECL = "TYPE_DECL"
    TYPE_PARAM_LIST = "TYPE_PARAM_LIST"
    VARIANT_LIST = "VARIANT_LIST"
    VARIANT = "VARIANT"
    TUPLE_FIELD_LIST = "TUPLE_FIELD_LIST"
    RECORD_FIELD_LIST = "RECORD_FIELD_LIST"
    RECORD_FIELD = "RECORD_FIELD"
    FN = "FN"
    CAST = "CAST"
    COMPONENT = "COMPONENT"
    LET_DECL = "LET_DECL"
    PARAM_LIST = "PARAM_LIST"
    PARAM = "PARAM"
    RET_TYPE = "RET_TYPE"

    # Nodes: types.
    NAMED_TYPE = "NAMED_TYPE"
    TYPE_ARG_LIST = "TYPE_ARG_LIST"
    FN_TYPE = "FN_TYPE"
    PARAM_TYPE_LIST = "PARAM_TYPE_LIST"
    PARAM_TYPE = "PARAM_TYPE"

    # Nodes: patterns.
    WILDCARD_PAT = "WILDCARD_PAT"
    IDENT_PAT = "IDENT_PAT"
    LITERAL_PAT = "LITERAL_PAT"
    CTOR_PAT = "CTOR_PAT"
    LIST_PAT = "LIST_PAT"
    REST_PAT = "REST_PAT"

    # Nodes: expressions and statements.
    LITERAL = "LITERAL"
    PATH_EXPR = "PATH_EXPR"
    HOLE_EXPR = "HOLE_EXPR"
    PAREN_EXPR = "PAREN_EXPR"
    LIST_EXPR = "LIST_EXPR"
    SPREAD = "SPREAD"
    BLOCK_EXPR = "BLOCK_EXPR"
    LET_STMT = "LET_STMT"
    EXPR_STMT = "EXPR_STMT"
    LAMBDA_EXPR = "LAMBDA_EXPR"
    IF_EXPR = "IF_EXPR"
    MATCH_EXPR = "MATCH_EXPR"
    MATCH_ARM_LIST = "MATCH_ARM_LIST"
    MATCH_ARM = "MATCH_ARM"
    MATCH_GUARD = "MATCH_GUARD"
    INTERPOLATED_STRING = "INTERPOLATED_STRING"
    BIN_EXPR = "BIN_EXPR"
    PREFIX_EXPR = "PREFIX_EXPR"
    CALL_EXPR = "CALL_EXPR"
    ARG_LIST = "ARG_LIST"
    LABELED_ARG = "LABELED_ARG"
    FIELD_EXPR = "FIELD_EXPR"
    TRY_EXPR = "TRY_EXPR"
    CAST_EXPR = "CAST_EXPR"

    def __str__(self) -> str:
        
        match self:
            case SyntaxKind.EOF:
                return "EOF"
            case SyntaxKind.LOWER_IDENT:
                return "a lowercase name"
            case SyntaxKind.UPPER_IDENT:
                return "a capitalized name"
            case SyntaxKind.INT_NUMBER | SyntaxKind.FLOAT_NUMBER:
                return "a number"
            case SyntaxKind.STRING | SyntaxKind.STRING_START:
                return "a string"
            case SyntaxKind.STRING_MIDDLE | SyntaxKind.STRING_END:
                return "`}`"
            case SyntaxKind.MODULE_KW:
                return "`module`"
            case SyntaxKind.IMPORT_KW:
                return "`import`"
            case SyntaxKind.TYPE_KW:
                return "`type`"
            case SyntaxKind.FN_KW:
                return "`fn`"
            case SyntaxKind.CAST_KW:
                return "`cast`"
            case SyntaxKind.COMPONENT_KW:
                return "`component`"
            case SyntaxKind.LET_KW:
                return "`let`"
            case SyntaxKind.MATCH_KW:
                return "`match`"
            case SyntaxKind.IF_KW:
                return "`if`"
            case SyntaxKind.ELSE_KW:
                return "`else`"
            case SyntaxKind.L_PAREN:
                return "`(`"
            case SyntaxKind.R_PAREN:
                return "`)`"
            case SyntaxKind.L_CURLY:
                return "`{`"
            case SyntaxKind.R_CURLY:
                return "`}`"
            case SyntaxKind.L_BRACK:
                return "`[`"
            case SyntaxKind.R_BRACK:
                return "`]`"
            case SyntaxKind.L_ANGLE:
                return "`<`"
            case SyntaxKind.R_ANGLE:
                return "`>`"
            case SyntaxKind.COMMA:
                return "`,`"
            case SyntaxKind.COLON:
                return "`:`"
            case SyntaxKind.COLON2:
                return "`::`"
            case SyntaxKind.DOT:
                return "`.`"
            case SyntaxKind.DOT2:
                return "`..`"
            case SyntaxKind.EQ:
                return "`=`"
            case SyntaxKind.EQ2:
                return "`==`"
            case SyntaxKind.NEQ:
                return "`!=`"
            case SyntaxKind.LTEQ:
                return "`<=`"
            case SyntaxKind.GTEQ:
                return "`>=`"
            case SyntaxKind.PIPE:
                return "`|`"
            case SyntaxKind.PIPE2:
                return "`||`"
            case SyntaxKind.PIPE_GT:
                return "`|>`"
            case SyntaxKind.AMP2:
                return "`&&`"
            case SyntaxKind.BANG:
                return "`!`"
            case SyntaxKind.THIN_ARROW:
                return "`->`"
            case SyntaxKind.FAT_ARROW:
                return "`=>`"
            case SyntaxKind.QUESTION:
                return "`?`"
            case SyntaxKind.PLUS:
                return "`+`"
            case SyntaxKind.PLUS2:
                return "`++`"
            case SyntaxKind.MINUS:
                return "`-`"
            case SyntaxKind.STAR:
                return "`*`"
            case SyntaxKind.STAR2:
                return "`**`"
            case SyntaxKind.SLASH:
                return "`/`"
            case SyntaxKind.TILDE_SLASH:
                return "`~/`"
            case SyntaxKind.PERCENT:
                return "`%`"
            case SyntaxKind.UNDERSCORE:
                return "`_`"
            case _:
                return "something else"

    def is_trivia(self) -> bool:
        """Whitespace and comments: in the tree, but invisible to the parser."""
        return self in (SyntaxKind.WHITESPACE, SyntaxKind.COMMENT)

    def is_keyword(self) -> bool:
        return self in (
            SyntaxKind.MODULE_KW,
            SyntaxKind.IMPORT_KW,
            SyntaxKind.TYPE_KW,
            SyntaxKind.FN_KW,
            SyntaxKind.CAST_KW,
            SyntaxKind.COMPONENT_KW,
            SyntaxKind.LET_KW,
            SyntaxKind.MATCH_KW,
            SyntaxKind.IF_KW,
            SyntaxKind.ELSE_KW,
        )

    @classmethod
    def from_keyword(cls, text: str) -> SyntaxKind | None:
        """The keyword spelled `text`, if it is one."""
        match text:
            case "module":
                return cls.MODULE_KW
            case "import":
                return cls.IMPORT_KW
            case "type":
                return cls.TYPE_KW
            case "fn":
                return cls.FN_KW
            case "cast":
                return cls.CAST_KW
            case "component":
                return cls.COMPONENT_KW
            case "let":
                return cls.LET_KW
            case "match":
                return cls.MATCH_KW
            case "if":
                return cls.IF_KW
            case "else":
                return cls.ELSE_KW
            case _:
                return None

    @classmethod
    def from_punct(cls, text: str) -> SyntaxKind | None:
        """The punctuation spelled `text`, if it is one."""
        match text:
            case "(":
                return cls.L_PAREN
            case ")":
                return cls.R_PAREN
            case "{":
                return cls.L_CURLY
            case "}":
                return cls.R_CURLY
            case "[":
                return cls.L_BRACK
            case "]":
                return cls.R_BRACK
            case "<":
                return cls.L_ANGLE
            case ">":
                return cls.R_ANGLE
            case ",":
                return cls.COMMA
            case ":":
                return cls.COLON
            case "::":
                return cls.COLON2
            case ".":
                return cls.DOT
            case "..":
                return cls.DOT2
            case "=":
                return cls.EQ
            case "==":
                return cls.EQ2
            case "!=":
                return cls.NEQ
            case "<=":
                return cls.LTEQ
            case ">=":
                return cls.GTEQ
            case "|":
                return cls.PIPE
            case "||":
                return cls.PIPE2
            case "|>":
                return cls.PIPE_GT
            case "&&":
                return cls.AMP2
            case "!":
                return cls.BANG
            case "->":
                return cls.THIN_ARROW
            case "=>":
                return cls.FAT_ARROW
            case "?":
                return cls.QUESTION
            case "+":
                return cls.PLUS
            case "++":
                return cls.PLUS2
            case "-":
                return cls.MINUS
            case "*":
                return cls.STAR
            case "**":
                return cls.STAR2
            case "/":
                return cls.SLASH
            case "~/":
                return cls.TILDE_SLASH
            case "%":
                return cls.PERCENT
            case _:
                return None

