from dataclasses import dataclass

from lexer.lex_error import LexError
from lexer.span import Span
from lexer.syntax_kind import SyntaxKind


@dataclass
class LexToken:
    text: str
    span: Span
    kind: SyntaxKind
    errors: LexError
