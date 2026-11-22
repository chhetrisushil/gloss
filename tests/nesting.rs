//! Deep nesting is reported or handled, never a stack overflow. These run on an ordinary
//! test thread (2 MB of stack), the smallest the compiler is expected to cope with.

use gloss::{check_source, format_source, parser::parse_expr_str};

fn nested_ifs(depth: usize) -> String {
    let mut text = String::from("feature start\n  set t to 1\n");
    for level in 0..depth {
        let pad = "  ".repeat(level + 1);
        text.push_str(&format!("{pad}if t is 1\n"));
    }
    text.push_str(&format!("{}show t\n", "  ".repeat(depth + 1)));
    text
}

fn parens(depth: usize) -> String {
    format!("{}1{}", "(".repeat(depth), ")".repeat(depth))
}

#[test]
fn parentheses_nest_one_hundred_deep_but_not_more() {
    assert!(parse_expr_str(&parens(100)).is_ok());
    let err = parse_expr_str(&parens(101)).unwrap_err();
    assert_eq!(err.problem, "This expression is nested too deeply");
}

#[test]
fn blocks_nest_one_hundred_deep_but_not_more() {
    let ok = nested_ifs(100);
    assert!(check_source(&ok).iter().all(|d| !d.is_error()));
    assert_eq!(format_source(&ok).unwrap(), ok);
    let err = format_source(&nested_ifs(101)).unwrap_err();
    assert_eq!(err.problem, "This block is nested too deeply");
    assert_eq!(check_source(&nested_ifs(101)).len(), 1);
    assert_eq!(check_source(&nested_ifs(3000)).len(), 1);
}

#[test]
fn a_ninety_nine_deep_expression_checks_and_formats() {
    let text = format!("feature start\n  show {}\n", parens(99));
    assert!(check_source(&text).iter().all(|d| !d.is_error()));
    assert_eq!(format_source(&text).unwrap(), text);
}

fn chain(parts: usize, op: &str) -> String {
    vec!["1"; parts].join(&format!(" {op} "))
}

#[test]
fn a_chain_of_operators_counts_toward_the_limit() {
    let too_many = "This expression has too many parts; split it into smaller steps with set";
    assert_eq!(
        parse_expr_str(&chain(101, "+")).unwrap_err().problem,
        too_many
    );
    assert_eq!(
        parse_expr_str(&chain(101, "*")).unwrap_err().problem,
        too_many
    );
    let ok = format!("feature start\n  show {}\n", chain(100, "+"));
    assert!(check_source(&ok).iter().all(|d| !d.is_error()));
    assert_eq!(format_source(&ok).unwrap().lines().count(), 2);
}

#[test]
fn very_long_chains_and_minus_signs_give_a_diagnostic() {
    for term in ["1 +", "1 -", "1 *", "1 /", "- ", "-"] {
        let text = format!("feature start\n  show {}1\n", term.repeat(20_000));
        assert_eq!(check_source(&text).len(), 1);
        assert!(format_source(&text).is_err());
    }
}

fn target_chain(parts: usize) -> String {
    format!("feature start\n  set {}x to 1\n", "a of ".repeat(parts))
}

fn item_target_chain(parts: usize) -> String {
    format!(
        "feature start\n  set {}x to 1\n",
        "item 1 of ".repeat(parts)
    )
}

fn remainder_chain(parts: usize) -> String {
    format!(
        "feature start\n  show {}7{}\n",
        "remainder of ".repeat(parts),
        " by 2".repeat(parts)
    )
}

const TOO_DEEP: &str = "This expression is nested too deeply";

#[test]
fn nested_assignment_targets_stop_at_one_hundred() {
    for build in [target_chain, item_target_chain] {
        let err = format_source(&build(101)).unwrap_err();
        assert_eq!(err.problem, TOO_DEEP);
        assert_eq!(check_source(&build(101)).len(), 1);
        assert_eq!(check_source(&build(300_000)).len(), 1);
        let ok = build(100);
        check_source(&ok);
        assert_eq!(format_source(&ok).unwrap(), ok);
    }
}

#[test]
fn nested_remainders_stop_at_one_hundred() {
    let err = format_source(&remainder_chain(101)).unwrap_err();
    assert_eq!(err.problem, TOO_DEEP);
    assert_eq!(check_source(&remainder_chain(200_000)).len(), 1);
    let ok = remainder_chain(100);
    check_source(&ok);
    assert_eq!(format_source(&ok).unwrap(), ok);
}
