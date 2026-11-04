from dataclasses import dataclass

from lexer.span import Span


@dataclass
class LexError(Exception):
    message: str
    span: Span
