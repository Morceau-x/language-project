//! An outline of a file: one line per declaration, its signature
//! without its body. A small client of the typed AST, like the outline
//! an editor shows next to the code.

use crate::ast::{self, AstNode};

/// The outline of `file`.
pub fn outline(file: &ast::SourceFile) -> String {
    todo!("Part 11b: outline")
}

/// `type Shape = Circle | Rect`, `type User = { name, age }`.
fn type_decl(decl: &ast::TypeDecl) -> String {
    todo!("Part 11b: type_decl")
}

/// `(s: Shape, _)`.
fn params(list: Option<ast::ParamList>) -> String {
    let params: Vec<String> = list
        .iter()
        .flat_map(|list| list.params())
        .map(|param| text(Some(param)))
        .collect();
    format!("({})", params.join(", "))
}

/// ` -> Float`, or nothing.
fn ret_type(ret: Option<ast::RetType>) -> String {
    match ret.and_then(|ret| ret.ty()) {
        Some(ty) => format!(" -> {}", text(Some(ty))),
        None => String::new(),
    }
}

/// The source text of a node, or `?` if it's missing.
fn text(node: Option<impl AstNode>) -> String {
    match node {
        Some(node) => node.syntax().text().trim().to_string(),
        None => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn every_kind_of_declaration() {
        let text = "\
module Shapes

import List

type Shape<a> =
  | Circle(Float)
  | Rect(Float, Float)

type Point = { x: Float, y: Float }

let origin: Point = Point(x: 0.0, y: 0.0)

let zero = 0

fn area(s: Shape<a>, _) -> Float {
  0.0
}

fn main() { 1 }

cast (d: Float) -> Celsius { Celsius(degrees: d) }

component Card(title: String)
";
        expect![[r#"
            module Shapes
            import List
            type Shape<a> = Circle | Rect
            type Point = { x, y }
            let origin: Point
            let zero
            fn area(s: Shape<a>, _) -> Float
            fn main()
            cast (d: Float) -> Celsius
            component Card(title: String)
        "#]]
        .assert_eq(&outline(&crate::parse(text).tree()));
    }

    #[test]
    fn missing_parts_show_as_question_marks() {
        expect![[r#"
            module M
            fn ?(x)
        "#]]
        .assert_eq(&outline(&crate::parse("module M\nfn (x").tree()));
    }
}
