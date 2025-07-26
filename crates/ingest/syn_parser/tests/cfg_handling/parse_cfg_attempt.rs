#![cfg(test)]
use std::collections::HashSet;

use parse_cfg::*;
use syn::Meta;
#[test]
fn test_from_docs() -> Result<(), ParseError> {
    let cfg: Cfg = r#"cfg(any(unix, feature = "extra"))"#.parse()?;
    assert_eq!(
        Cfg::Any(vec![
            Cfg::Is("unix".into()),
            Cfg::Equal("feature".into(), "extra".into()),
        ]),
        cfg
    );

    let is_set = cfg.eval(|key, comparison| {
        if key == "feature" && comparison == "extra" {
            Some(comparison)
        } else {
            None
        }
    });
    assert!(is_set);

    let target = "powerpc64le-unknown-linux-gnu".parse()?;
    assert_eq!(
        Target::Triple {
            arch: "powerpc64le".into(),
            vendor: "unknown".into(),
            os: "linux".into(),
            env: Some("gnu".into()),
        },
        target
    );

    /// `Cfg` and `Target` types take an optional generic argument for the string type,
    /// so you can parse slices without allocating `String`s, or parse into `Cow<str>`.
    let target = Target::<&str>::parse_generic("powerpc64le-unknown-linux-gnu")?;
    assert_eq!(
        Target::Triple {
            arch: "powerpc64le",
            vendor: "unknown",
            os: "linux",
            env: Some("gnu"),
        },
        target
    );

    Ok(())
}
/// Everything we can encounter inside a `#[cfg(…)]`
#[derive(Debug)]
enum CfgAtom {
    Feature(String),
    TargetOs(String),
    // … add whatever else you need …
}

/// Parsed cfg expression tree
#[derive(Debug)]
enum CfgExpr {
    All(Vec<CfgExpr>),
    Any(Vec<CfgExpr>),
    Not(Box<CfgExpr>),
    Atom(CfgAtom),
}

/// Active flags as seen by Cargo
struct ActiveCfg {
    features: HashSet<String>,
    target_os: String,
}

impl ActiveCfg {
    fn eval(&self, expr: &CfgExpr) -> bool {
        match expr {
            CfgExpr::All(children) => children.iter().all(|c| self.eval(c)),
            CfgExpr::Any(children) => children.iter().any(|c| self.eval(c)),
            CfgExpr::Not(inner) => !self.eval(inner),
            CfgExpr::Atom(atom) => match atom {
                CfgAtom::Feature(f) => self.features.contains(f),
                CfgAtom::TargetOs(os) => self.target_os == *os,
            },
        }
    }
}

// Create a set of tests using `syn` to tokenize and then evaluate against the following string
// using the approaches in `CfgAtom`, `CfgExpr`, `ActiveCfg`!
const TEST_CFG_EVAL_TARGET: &str = r#"
// --- Scenario 12 & 13: Mutually Exclusive `cfg` Attributes ---
#[cfg(feature = "cfg_a")]
pub mod cfg_mod {
    pub fn item_in_cfg_a() -> u8 {
        120
    }
    #[cfg(feature = "cfg_b")]
    pub mod nested_cfg {
        pub fn item_in_cfg_ab() -> u8 {
            130
        }
    }
}

#[cfg(not(feature = "cfg_a"))]
pub mod cfg_mod {
    // Same name, different NodeId due to cfg
    pub fn item_in_cfg_not_a() -> u8 {
        121
    }
    #[cfg(feature = "cfg_c")]
    pub mod nested_cfg {
        // Same name, different NodeId
        pub fn item_in_cfg_nac() -> u8 {
            131
        }
    }
}
"#;
/// Parse a `#[cfg(...)]` attribute into our CfgExpr
fn parse_cfg_attribute(attr: &syn::Attribute) -> Option<CfgExpr> {
    if !attr.path().is_ident("cfg") {
        return None;
    }
    let Meta::List(list) = &attr.meta else { return None };
    parse_cfg_list(list)
}

fn parse_cfg_list(list: &syn::MetaList) -> Option<CfgExpr> {
    let mut iter = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
    ).ok()?;

    if iter.len() == 1 {
        parse_single_meta(iter.pop()?.into_value())
    } else {
        // cfg(feature = "a", feature = "b")  => treat as cfg(any(...))
        let args: Vec<CfgExpr> = iter.into_iter().filter_map(parse_single_meta).collect();
        Some(CfgExpr::Any(args))
    }
}

fn parse_single_meta(meta: syn::Meta) -> Option<CfgExpr> {
    match meta {
        Meta::Path(path) => Some(CfgExpr::Atom(CfgAtom::Feature(
            path.get_ident()?.to_string(),
        ))),
        Meta::NameValue(nv) => {
            let key = nv.path.get_ident()?.to_string();
            let value = match nv.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                _ => return None,
            };
            match key.as_str() {
                "feature" => Some(CfgExpr::Atom(CfgAtom::Feature(value))),
                "target_os" => Some(CfgExpr::Atom(CfgAtom::TargetOs(value))),
                _ => None,
            }
        }
        Meta::List(list) => {
            let ident = list.path.get_ident()?.to_string();
            let args: Vec<CfgExpr> = list
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )
                .ok()?
                .into_iter()
                .filter_map(parse_single_meta)
                .collect();
            match ident.as_str() {
                "all" => Some(CfgExpr::All(args)),
                "any" => Some(CfgExpr::Any(args)),
                "not" => args.into_iter().next().map(|e| CfgExpr::Not(Box::new(e))),
                _ => None,
            }
        }
    }
}

#[test]
fn test_cfg_evaluation() {
    // Parse the test file
    let file = parse_file(TEST_CFG_EVAL_TARGET).expect("Failed to parse test file");

    // Build active configurations to test
    let active_with_cfg_a = ActiveCfg {
        features: ["cfg_a".to_string()].into_iter().collect(),
        target_os: "linux".to_string(),
    };

    let active_without_cfg_a = ActiveCfg {
        features: [].into_iter().collect(),
        target_os: "linux".to_string(),
    };

    let active_with_cfg_b = ActiveCfg {
        features: ["cfg_a".to_string(), "cfg_b".to_string()]
            .into_iter()
            .collect(),
        target_os: "linux".to_string(),
    };

    let active_with_cfg_c = ActiveCfg {
        features: ["cfg_c".to_string()].into_iter().collect(),
        target_os: "linux".to_string(),
    };

    // Find all module items and their cfg attributes
    let mut modules = Vec::new();
    for item in file.items {
        if let Item::Mod(item_mod) = item {
            let cfgs: Vec<CfgExpr> = item_mod
                .attrs
                .iter()
                .filter_map(|attr| parse_cfg_attribute(attr))
                .collect();
            modules.push((item_mod, cfgs));
        }
    }

    // Test scenarios
    assert_eq!(modules.len(), 2, "Expected two cfg_mod variants");

    // First module should be active when cfg_a is enabled
    let first_mod_active = modules[0].1.iter().all(|cfg| active_with_cfg_a.eval(cfg));
    assert!(
        first_mod_active,
        "First cfg_mod should be active with cfg_a enabled"
    );

    // Second module should be active when cfg_a is NOT enabled
    let second_mod_active = modules[1]
        .1
        .iter()
        .all(|cfg| active_without_cfg_a.eval(cfg));
    assert!(
        second_mod_active,
        "Second cfg_mod should be active when cfg_a is disabled"
    );

    // Test nested cfg scenarios
    let mut nested_modules = Vec::new();
    for (_, cfgs) in &modules {
        // In a real implementation, we'd recurse into the module contents
        // For this test, we'll just verify the cfg evaluation works
        for cfg in cfgs {
            if let CfgExpr::Not(inner) = cfg {
                if let CfgExpr::Atom(CfgAtom::Feature(f)) = &**inner {
                    assert_eq!(f, "cfg_a");
                }
            }
        }
    }

    // Test feature combinations
    let first_mod_with_b = modules[0].1.iter().all(|cfg| active_with_cfg_b.eval(cfg));
    assert!(
        first_mod_with_b,
        "First cfg_mod should still be active with cfg_a an
 cfg_b"
    );

    let second_mod_with_c = modules[1].1.iter().all(|cfg| active_with_cfg_c.eval(cfg));
    assert!(
        !second_mod_with_c,
        "Second cfg_mod should not be active with only
 cfg_c"
    );
}
