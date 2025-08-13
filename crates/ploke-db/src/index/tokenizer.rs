// ploke_code_tokenizer.rs
// A small, self-contained code-aware tokenizer for Ploke.
// - Splits identifiers on snake_case and camelCase / PascalCase and handles simple acronyms
// - Extracts line (//) and block (/* */) comments
// - Tokenizes code into identifiers, symbols, and comment tokens

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TokenType {
    Identifier,
    Symbol,
    Comment,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Token {
    pub text: String,
    pub kind: TokenType,
}

pub struct CodeTokenizer;

impl CodeTokenizer {
    /// Tokenize a code snippet into tokens (identifiers, symbols, comments).
    /// This is intentionally conservative and dependency-free so it can be embedded easily.
    pub fn tokenize(snippet: &str) -> Vec<Token> {
        let mut tokens = Vec::new();

        // First, extract block comments and replace them with spaces so they don't interfere
        // with tokenization of code. We keep the comment text as separate tokens.
        let (without_block_comments, mut block_comments) = Self::strip_block_comments(snippet);
        for c in block_comments.drain(..) {
            tokens.push(Token {
                text: c,
                kind: TokenType::Comment,
            });
        }

        // Then process line-by-line to extract line comments and tokenize code part.
        for line in without_block_comments.lines() {
            if let Some(idx) = line.find("//") {
                let (code_part, comment_part) = line.split_at(idx);
                // tokenize code part
                tokens.extend(Self::tokenize_code_part(code_part));
                // push comment (drop the leading //)
                let comment = comment_part.trim_start_matches('/').to_string();
                tokens.push(Token {
                    text: comment,
                    kind: TokenType::Comment,
                });
            } else {
                tokens.extend(Self::tokenize_code_part(line));
            }
        }

        tokens
    }

    /// Remove block comments and return (code_without_block_comments, vec_of_block_comments)
    fn strip_block_comments(src: &str) -> (String, Vec<String>) {
        let mut out = String::with_capacity(src.len());
        let mut comments = Vec::new();
        let bytes = src.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                // found block comment
                let start = i + 2;
                i = start;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                let end = if i + 1 < bytes.len() { i } else { bytes.len() };
                let comment = String::from_utf8_lossy(&bytes[start..end]).to_string();
                comments.push(comment);
                // skip the closing */ if present
                if i + 1 < bytes.len() {
                    i += 2;
                }
                // replace comment area in output with a single space to keep line/column alignment
                out.push(' ');
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        (out, comments)
    }

    /// Tokenize a single code line (without block comments). This treats // comments as removed.
    fn tokenize_code_part(line: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut cur = String::new();
        let mut cur_is_id = false;

        let push_cur = |tokens: &mut Vec<Token>, cur: &mut String, cur_is_id: &mut bool| {
            if cur.is_empty() {
                return;
            }
            if *cur_is_id {
                // split identifier into subtokens
                let subs = split_identifier(cur);
                for s in subs {
                    tokens.push(Token {
                        text: s,
                        kind: TokenType::Identifier,
                    });
                }
            } else {
                tokens.push(Token {
                    text: cur.clone(),
                    kind: TokenType::Symbol,
                });
            }
            cur.clear();
            *cur_is_id = false;
        };

        for ch in line.chars() {
            if is_ident_char(ch) {
                if !cur_is_id {
                    // flush symbol buffer first
                    push_cur(&mut tokens, &mut cur, &mut cur_is_id);
                }
                cur.push(ch);
                cur_is_id = true;
            } else if ch.is_whitespace() {
                push_cur(&mut tokens, &mut cur, &mut cur_is_id);
            } else {
                // punctuation / symbol
                if cur_is_id {
                    push_cur(&mut tokens, &mut cur, &mut cur_is_id);
                }
                let mut s = String::new();
                s.push(ch);
                tokens.push(Token {
                    text: s,
                    kind: TokenType::Symbol,
                });
            }
        }
        push_cur(&mut tokens, &mut cur, &mut cur_is_id);
        tokens
    }
}

/// Heuristic: identifier characters are alphanumeric or underscore
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Split an identifier on snake_case and camelCase boundaries.
/// Examples:
/// - `snake_case` -> ["snake", "case"]
/// - `camelCase` -> ["camel", "case"]
/// - `PascalCase` -> ["pascal", "case"]
/// - `HTTPServer` -> ["http", "server"]
fn split_identifier(ident: &str) -> Vec<String> {
    // First split on underscores
    let mut parts: Vec<String> = Vec::new();
    for chunk in ident.split('_') {
        if chunk.is_empty() {
            continue;
        }
        // split camel/pascal in chunk
        let mut cur = String::new();
        let chars: Vec<char> = chunk.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if i > 0 {
                let prev = chars[i - 1];
                let next = chars.get(i + 1).copied();
                // boundary: lower->upper (fooBar)
                let lower_to_upper = prev.is_lowercase() && ch.is_uppercase();
                // boundary: acronym end: UPPER + Upper + lower (HTTPTok -> HTTP Tok)
                let upper_seq_then_lower = prev.is_uppercase()
                    && ch.is_uppercase()
                    && next.map_or_else(|| false, |n| n.is_lowercase());
                // boundary: digit transitions
                let digit_boundary = (prev.is_ascii_digit() && !ch.is_ascii_digit())
                    || (!prev.is_ascii_digit() && ch.is_ascii_digit());
                if (lower_to_upper || upper_seq_then_lower || digit_boundary) && !cur.is_empty() {
                    parts.push(cur.to_lowercase());
                    cur.clear();
                }
            }
            cur.push(ch);
        }
        if !cur.is_empty() {
            parts.push(cur.to_lowercase());
        }
    }
    parts
}

// -------------------- Tests --------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_snake_case() {
        let got = split_identifier("snake_case_example");
        let want = vec!["snake", "case", "example"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(got, want);
    }

    #[test]
    fn split_camel_case() {
        let got = split_identifier("camelCaseExample");
        let want = vec!["camel", "case", "example"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(got, want);
    }

    #[test]
    fn split_pascal_and_acronym() {
        let got = split_identifier("HTTPServerError");
        let want = vec!["http", "server", "error"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(got, want);
    }

    #[test]
    fn tokenize_comments_and_code() {
        let src = r#"
        // this is a line comment
        fn foo_bar(x: i32) -> i32 { /* block comment */ x + 1 }
        "#;
        let toks = CodeTokenizer::tokenize(src);
        // we expect at least one comment token and some identifier subtokens
        let has_comment = toks.iter().any(|t| t.kind == TokenType::Comment);
        assert!(has_comment, "expected a comment token");

        // find identifier subtokens for foo_bar -> foo, bar
        let idents: Vec<&String> = toks
            .iter()
            .filter_map(|t| {
                if t.kind == TokenType::Identifier {
                    Some(&t.text)
                } else {
                    None
                }
            })
            .collect();
        assert!(idents.contains(&&"foo".to_string()));
        assert!(idents.contains(&&"bar".to_string()));
    }

    #[test]
    fn tokenize_mixed_symbols() {
        let src = "let result = myVar123 + 456;";
        let toks = CodeTokenizer::tokenize(src);
        let identifiers: Vec<String> = toks
            .iter()
            .filter(|t| t.kind == TokenType::Identifier)
            .map(|t| t.text.clone())
            .collect();
        assert!(identifiers.contains(&"let".to_string()));
        assert!(identifiers.contains(&"result".to_string()));
        assert!(identifiers.contains(&"my".to_string())); // myVar123 -> ["my","var","123"]
        assert!(identifiers.contains(&"var".to_string()));
        assert!(identifiers.contains(&"123".to_string()));
    }
}
