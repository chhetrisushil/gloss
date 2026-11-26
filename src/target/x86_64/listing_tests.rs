//! Tests of the assembly listing.

use super::inst::{X, text};
use super::listing::*;
use super::program::assemble;
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
