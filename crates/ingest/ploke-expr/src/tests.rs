use std::fmt::Write;

use expect_test::expect;

use crate::{
    Edition, LexedStr, PrefixEntryPoint, Step, StrStep, SyntaxKind, TopEntryPoint, parse_expr_cst,
    parse_source_file_cst,
};

#[test]
fn parse_smoke_test() {
    let code = r#"
fn main() {
    println!("Hello, world!")
}
    "#;

    let parse = parse_source_file_cst(code, Edition::CURRENT);
    assert!(parse.errors().is_empty(), "{:?}", parse.errors());
}

#[test]
fn parse_source_file_cst_preserves_text_and_root_kind() {
    let text = "//! docs\nfn demo() {\n    // comment\n    let value = 1 + 2;\n}\n";

    let parse = parse_source_file_cst(text, Edition::CURRENT);
    let root = parse.syntax_node();

    assert_eq!(root.kind(), SyntaxKind::SOURCE_FILE);
    assert_eq!(root.text().to_string(), text);
    assert!(parse.errors().is_empty(), "{:?}", parse.errors());
}

#[test]
fn parse_expr_cst_materializes_valid_expression_tree() {
    let text = "foo::bar::<u8>(1 + 2)";

    let parse = parse_expr_cst(text, Edition::CURRENT);
    let root = parse.syntax_node();

    assert_eq!(root.kind(), SyntaxKind::CALL_EXPR);
    assert_eq!(root.text().to_string(), text);
    assert!(parse.errors().is_empty(), "{:?}", parse.errors());
}

#[test]
fn parse_expr_cst_handles_field_chain_after_float_split() {
    let text = "foo.0.1";

    let parse = parse_expr_cst(text, Edition::CURRENT);
    let root = parse.syntax_node();

    assert_eq!(root.kind(), SyntaxKind::FIELD_EXPR);
    assert_eq!(root.text().to_string(), text);
    assert!(parse.errors().is_empty(), "{:?}", parse.errors());
}

#[test]
fn parse_expr_cst_reports_invalid_expression_errors() {
    let parse = parse_expr_cst("foo(", Edition::CURRENT);

    assert!(!parse.errors().is_empty(), "expected parse errors for invalid expression");
}

#[test]
fn ra_prefix_expr_entry_consumes_expected_prefix() {
    check_prefix_entry(PrefixEntryPoint::Expr, "92 92", "92");
    check_prefix_entry(PrefixEntryPoint::Expr, "+1", "+");
    check_prefix_entry(PrefixEntryPoint::Expr, "-1", "-1");
    check_prefix_entry(PrefixEntryPoint::Expr, "fn foo() {}", "fn");
    check_prefix_entry(PrefixEntryPoint::Expr, "#[attr] ()", "#[attr] ()");
    check_prefix_entry(PrefixEntryPoint::Expr, "foo.0", "foo.0");
    check_prefix_entry(PrefixEntryPoint::Expr, "foo.0.1", "foo.0.1");
    check_prefix_entry(PrefixEntryPoint::Expr, "foo.0. foo", "foo.0. foo");
}

#[test]
fn ra_top_expr_entry_matches_expected_tree() {
    check_top_entry(
        TopEntryPoint::Expr,
        "",
        expect![[r#"
            ERROR
            error 0: expected expression
        "#]],
    );
    check_top_entry(
        TopEntryPoint::Expr,
        "2 + 2 == 5",
        expect![[r#"
        BIN_EXPR
          BIN_EXPR
            LITERAL
              INT_NUMBER "2"
            WHITESPACE " "
            PLUS "+"
            WHITESPACE " "
            LITERAL
              INT_NUMBER "2"
          WHITESPACE " "
          EQ2 "=="
          WHITESPACE " "
          LITERAL
            INT_NUMBER "5"
    "#]],
    );
    check_top_entry(
        TopEntryPoint::Expr,
        "let _ = 0;",
        expect![[r#"
            ERROR
              LET_EXPR
                LET_KW "let"
                WHITESPACE " "
                WILDCARD_PAT
                  UNDERSCORE "_"
                WHITESPACE " "
                EQ "="
                WHITESPACE " "
                LITERAL
                  INT_NUMBER "0"
              SEMICOLON ";"
        "#]],
    );
}

#[track_caller]
fn check_prefix_entry(entry: PrefixEntryPoint, input: &str, prefix: &str) {
    let lexed = LexedStr::new(Edition::CURRENT, input);
    let input = lexed.to_input(Edition::CURRENT);

    let mut n_tokens = 0;
    for step in entry.parse(&input).iter() {
        match step {
            Step::Token { n_input_tokens, .. } => n_tokens += n_input_tokens as usize,
            Step::FloatSplit { .. } => n_tokens += 1,
            Step::Enter { .. } | Step::Exit | Step::Error { .. } => (),
        }
    }

    let mut i = 0;
    loop {
        if n_tokens == 0 {
            break;
        }
        if !lexed.kind(i).is_trivia() {
            n_tokens -= 1;
        }
        i += 1;
    }
    let buf = &lexed.as_str()[..lexed.text_start(i)];
    assert_eq!(buf, prefix);
}

#[track_caller]
fn check_top_entry(entry: TopEntryPoint, input: &str, expected: expect_test::Expect) {
    let actual = parse_debug(entry, input, Edition::CURRENT).0;
    expected.assert_eq(&actual);
}

fn parse_debug(entry: TopEntryPoint, text: &str, edition: Edition) -> (String, bool) {
    let lexed = LexedStr::new(edition, text);
    let input = lexed.to_input(edition);
    let output = entry.parse(&input);

    let mut buf = String::new();
    let mut errors = Vec::new();
    let mut indent = String::new();
    let mut depth = 0;
    let mut len = 0;
    lexed.intersperse_trivia(&output, &mut |step| match step {
        StrStep::Token { kind, text } => {
            assert!(depth > 0);
            len += text.len();
            writeln!(buf, "{indent}{kind:?} {text:?}").unwrap();
        }
        StrStep::Enter { kind } => {
            assert!(depth > 0 || len == 0);
            depth += 1;
            writeln!(buf, "{indent}{kind:?}").unwrap();
            indent.push_str("  ");
        }
        StrStep::Exit => {
            assert!(depth > 0);
            depth -= 1;
            indent.pop();
            indent.pop();
        }
        StrStep::Error { msg, pos } => {
            assert!(depth > 0);
            errors.push(format!("error {pos}: {msg}\n"));
        }
    });
    assert_eq!(
        len,
        text.len(),
        "didn't parse all text.\nParsed:\n{}\n\nAll:\n{}\n",
        &text[..len],
        text
    );

    for (token, msg) in lexed.errors() {
        let pos = lexed.text_start(token);
        errors.push(format!("error {pos}: {msg}\n"));
    }

    let has_errors = !errors.is_empty();
    for error in errors {
        buf.push_str(&error);
    }
    (buf, has_errors)
}
