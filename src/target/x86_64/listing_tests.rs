//! Tests of the assembly listing.

use super::inst::{X, text};
use super::listing::*;
use super::program::{Assembled, Assembly, assemble};
use crate::ir::Module;
use crate::ir::{Function, Inst, Label};
use crate::target::listing::Item;

fn module(src: &str) -> Module {
    let (_, checked) = crate::check_program(src);
    let c = checked.unwrap();
    crate::lower::lower(&c.program, &c.types, &c.src).unwrap()
}

#[test]
fn the_listing_has_one_instruction_per_assembled_instruction() {
    let m = module(
        "feature start\n  for each i from 1 to 3\n    if i is 2\n      show \"two\"\n    otherwise\n      show i\n",
    );
    let a = assemble(&m).unwrap();
    let routines = listing(&a, &m).unwrap();
    assert_eq!(routines.len(), a.routines.len());
    for (r, asm) in routines.iter().zip(&a.routines) {
        let got: Vec<String> = r
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Inst { text, .. } => Some(text.replace(".L", &format!("L{}_", r.name))),
                _ => None,
            })
            .collect();
        let want: Vec<String> = asm
            .code
            .iter()
            .filter(|x| !matches!(x, X::Label(_) | X::Comment(_)))
            .map(text)
            .collect();
        assert_eq!(got, want, "routine {}", r.name);
    }
}

fn one(body: Vec<Inst>) -> Vec<Item> {
    let f = Function {
        name: "f".to_string(),
        params: 0,
        slots: 0,
        body,
    };
    let m = Module {
        functions: vec![f],
        texts: vec![],
        descs: vec![],
    };
    listing(&assemble(&m).unwrap(), &m).unwrap().remove(0).items
}

#[test]
fn line_markers_and_local_labels() {
    let items = one(vec![
        Inst::Line(3),
        Inst::Mark(Label(1)),
        Inst::Jump(Label(1)),
    ]);
    assert!(!items.contains(&Item::Label {
        text: "f".to_string(),
        local: None
    }));
    let at = items.iter().position(|i| *i == Item::Line(3)).unwrap();
    assert_eq!(
        items[at + 1],
        Item::Label {
            text: ".L1".to_string(),
            local: Some(1)
        }
    );
    assert_eq!(
        items[at + 2],
        Item::Inst {
            text: "jmp .L1".to_string(),
            refs: vec![]
        }
    );
}

#[test]
fn helper_labels_stay_as_they_are() {
    let m = module("feature start\n  show 1\n");
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    let helpers = routines.iter().filter(|r| r.name.starts_with("x86_64."));
    let labels: Vec<&Item> = helpers
        .flat_map(|r| &r.items)
        .filter(|i| matches!(i, Item::Label { .. }))
        .collect();
    assert!(labels.contains(&&Item::Label {
        text: "x86_64.number_finish".to_string(),
        local: None
    }));
    assert!(
        labels
            .iter()
            .all(|i| matches!(i, Item::Label { local: None, .. }))
    );
}

#[test]
fn an_unknown_comment_is_an_error() {
    let a = Assembly {
        routines: vec![Assembled {
            name: "f".to_string(),
            code: vec![X::Label("f".to_string()), X::Comment("hi".to_string())],
        }],
    };
    let m = Module {
        functions: vec![],
        texts: vec![],
        descs: vec![],
    };
    assert!(listing(&a, &m).unwrap_err().contains("unexpected comment"));
}

#[test]
fn only_a_routines_own_numbered_labels_are_local() {
    assert_eq!(local("f_a", "Lf_a_3"), Some(3));
    assert_eq!(local("f_a", "Lf_a_b_3"), None);
    assert_eq!(local("f_a", "Lf_a_"), None);
    assert_eq!(local("f_a", "Lf_a3"), None);
    assert_eq!(local("f_a", "Lf_b_3"), None);
}

#[test]
fn conditional_jumps_are_renamed_and_calls_are_not() {
    let items = one(vec![
        Inst::Mark(Label(2)),
        Inst::Compare,
        Inst::JumpIf(crate::ir::Cond::Eq, Label(2)),
    ]);
    assert!(items.contains(&Item::Inst {
        text: "je .L2".to_string(),
        refs: vec![]
    }));
    let m = module("feature start\n  show 1\n");
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    let calls = routines
        .iter()
        .flat_map(|r| &r.items)
        .filter_map(|i| match i {
            Item::Inst { text, .. } if text.starts_with("call ") => Some(text),
            _ => None,
        });
    assert!(calls.clone().count() > 0);
    assert!(calls.into_iter().all(|t| !t.starts_with("call .L")));
}

#[test]
fn system_call_labels_in_runtime_routines_stay_as_they_are() {
    let m = module("feature start\n  set xs to list of 1, 2\n  show xs\n");
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    let mapped: Vec<&Item> = routines
        .iter()
        .filter(|r| !r.name.starts_with("x86_64."))
        .flat_map(|r| &r.items)
        .filter(|i| matches!(i, Item::Label { text, .. } if text.ends_with("_mapped")))
        .collect();
    assert!(!mapped.is_empty());
    assert!(
        mapped
            .iter()
            .all(|i| matches!(i, Item::Label { local: None, .. }))
    );
}
