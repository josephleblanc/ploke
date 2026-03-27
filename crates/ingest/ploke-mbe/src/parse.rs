use std::iter::Peekable;

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};
use quote::ToTokens;

use crate::error::MbeError;
use crate::ir::{
    DeclarativeMacro, MacroInvocation, MetaTemplate, MetaVarKind, Op, RepeatKind, Rule, Separator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pattern,
    Template,
}

pub fn parse_macro_rules_item(item: &syn::ItemMacro) -> Result<DeclarativeMacro, MbeError> {
    if !item.mac.path.is_ident("macro_rules") {
        return Err(MbeError::NotMacroRules {
            found: item.mac.path.to_token_stream().to_string(),
        });
    }

    let name = item
        .ident
        .as_ref()
        .map(ToString::to_string)
        .ok_or(MbeError::MissingMacroName)?;

    parse_macro_rules_tokens(name, item.mac.tokens.clone())
}

pub fn parse_macro_rules_tokens(
    name: impl Into<String>,
    tokens: TokenStream,
) -> Result<DeclarativeMacro, MbeError> {
    let rules = split_rules(tokens)?
        .into_iter()
        .map(parse_rule)
        .collect::<Result<Vec<_>, _>>()?;

    if rules.is_empty() {
        return Err(MbeError::InvalidRuleSyntax {
            message: "macro has no rules".into(),
        });
    }

    Ok(DeclarativeMacro {
        name: name.into(),
        rules,
    })
}

pub fn parse_invocation(item: &syn::ItemMacro) -> MacroInvocation {
    MacroInvocation {
        path: item.mac.path.to_token_stream().to_string(),
        tokens: item.mac.tokens.clone(),
    }
}

fn split_rules(tokens: TokenStream) -> Result<Vec<TokenStream>, MbeError> {
    let mut rules = Vec::new();
    let mut current = TokenStream::new();

    for tt in tokens {
        match &tt {
            TokenTree::Punct(punct)
                if punct.as_char() == ';' && punct.spacing() == Spacing::Alone =>
            {
                if !current.is_empty() {
                    rules.push(current);
                    current = TokenStream::new();
                }
            }
            _ => current.extend(std::iter::once(tt)),
        }
    }

    if !current.is_empty() {
        rules.push(current);
    }

    Ok(rules)
}

fn parse_rule(tokens: TokenStream) -> Result<Rule, MbeError> {
    let mut iter = tokens.into_iter().peekable();
    let matcher_tokens = collect_until_fat_arrow(&mut iter)?;
    let transcriber_tokens: TokenStream = iter.collect();

    if transcriber_tokens.is_empty() {
        return Err(MbeError::InvalidRuleSyntax {
            message: "rule is missing transcriber after `=>`".into(),
        });
    }

    Ok(Rule {
        matcher: parse_template(matcher_tokens, Mode::Pattern)?,
        transcriber: parse_template(transcriber_tokens, Mode::Template)?,
    })
}

fn collect_until_fat_arrow(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
) -> Result<TokenStream, MbeError> {
    let mut matcher = TokenStream::new();

    loop {
        let Some(tt) = iter.next() else {
            return Err(MbeError::InvalidRuleSyntax {
                message: "rule is missing `=>`".into(),
            });
        };

        if let TokenTree::Punct(eq) = &tt
            && eq.as_char() == '='
            && eq.spacing() == Spacing::Joint
            && matches!(iter.peek(), Some(TokenTree::Punct(gt)) if gt.as_char() == '>')
        {
            iter.next();
            break;
        }

        matcher.extend(std::iter::once(tt));
    }

    Ok(matcher)
}

fn parse_template(tokens: TokenStream, mode: Mode) -> Result<MetaTemplate, MbeError> {
    let mut iter = tokens.into_iter().peekable();
    let mut ops = Vec::new();

    while iter.peek().is_some() {
        ops.push(parse_op(&mut iter, mode)?);
    }

    Ok(MetaTemplate { ops })
}

fn parse_op(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
    mode: Mode,
) -> Result<Op, MbeError> {
    let tt = iter.next().ok_or(MbeError::UnexpectedEnd {
        context: "macro template",
    })?;

    match tt {
        TokenTree::Punct(punct) if punct.as_char() == '$' => parse_dollar_construct(iter, mode),
        TokenTree::Group(group) => Ok(Op::Subtree {
            delimiter: group.delimiter(),
            tokens: parse_template(group.stream(), mode)?,
        }),
        TokenTree::Ident(ident) => Ok(Op::Ident(ident.to_string())),
        TokenTree::Literal(literal) => Ok(Op::Literal(literal.to_string())),
        TokenTree::Punct(punct) => Ok(Op::Punct(punct.as_char())),
    }
}

fn parse_dollar_construct(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
    mode: Mode,
) -> Result<Op, MbeError> {
    let next = iter.next().ok_or(MbeError::UnexpectedEnd {
        context: "dollar construct",
    })?;

    match next {
        TokenTree::Ident(ident) => parse_metavar(iter, mode, ident),
        TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
            parse_repeat(iter, mode, group)
        }
        other => Err(MbeError::UnsupportedSyntax {
            message: format!("unsupported token after `$`: `{}`", other),
        }),
    }
}

fn parse_metavar(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
    mode: Mode,
    ident: Ident,
) -> Result<Op, MbeError> {
    let name = ident.to_string();

    if mode == Mode::Pattern
        && matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == ':')
    {
        iter.next();
        let kind_ident = match iter.next() {
            Some(TokenTree::Ident(kind)) => kind,
            Some(other) => {
                return Err(MbeError::UnsupportedSyntax {
                    message: format!(
                        "expected fragment specifier after `:${name}`, found `{other}`"
                    ),
                });
            }
            None => {
                return Err(MbeError::UnexpectedEnd {
                    context: "fragment specifier",
                });
            }
        };
        let fragment = kind_ident.to_string();
        let kind =
            MetaVarKind::parse(&fragment).ok_or_else(|| MbeError::InvalidFragment { fragment })?;
        return Ok(Op::Var {
            name,
            kind: Some(kind),
        });
    }

    Ok(Op::Var { name, kind: None })
}

fn parse_repeat(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
    mode: Mode,
    group: Group,
) -> Result<Op, MbeError> {
    let repeated = parse_template(group.stream(), mode)?;

    let (separator, kind) = match iter.peek() {
        Some(TokenTree::Punct(punct)) if is_repeat_kind(punct.as_char()) => {
            let punct = take_punct(iter, "repeat operator")?;
            (None, parse_repeat_kind(punct)?)
        }
        Some(_) => {
            let separator = iter.next().ok_or(MbeError::UnexpectedEnd {
                context: "repeat separator",
            })?;
            let kind = parse_repeat_kind(take_punct(iter, "repeat operator")?)?;
            (
                Some(Separator {
                    text: separator.to_string(),
                }),
                kind,
            )
        }
        None => {
            return Err(MbeError::UnexpectedEnd {
                context: "repeat operator",
            });
        }
    };

    Ok(Op::Repeat {
        tokens: repeated,
        separator,
        kind,
    })
}

fn take_punct(
    iter: &mut Peekable<proc_macro2::token_stream::IntoIter>,
    context: &'static str,
) -> Result<Punct, MbeError> {
    match iter.next() {
        Some(TokenTree::Punct(punct)) => Ok(punct),
        Some(other) => Err(MbeError::UnsupportedSyntax {
            message: format!("expected punctuation for {context}, found `{other}`"),
        }),
        None => Err(MbeError::UnexpectedEnd { context }),
    }
}

fn is_repeat_kind(punct: char) -> bool {
    matches!(punct, '*' | '+' | '?')
}

fn parse_repeat_kind(punct: Punct) -> Result<RepeatKind, MbeError> {
    match punct.as_char() {
        '*' => Ok(RepeatKind::ZeroOrMore),
        '+' => Ok(RepeatKind::OneOrMore),
        '?' => Ok(RepeatKind::ZeroOrOne),
        other => Err(MbeError::UnsupportedSyntax {
            message: format!("unsupported repeat operator `{other}`"),
        }),
    }
}
