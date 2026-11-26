//! Tests of the assembly listing.

use super::inst::{Mem, R, X, text};
use super::listing::*;
use super::program::{Assembled, Assembly, assemble};
use crate::ir::Module;
use crate::ir::{Function, Inst, Label};
use crate::target::listing::{Item, Ref};

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
    shaped(0, 0, body)
}

fn shaped(params: u32, slots: u32, body: Vec<Inst>) -> Vec<Item> {
    let f = Function {
        name: "f".to_string(),
        params,
        slots,
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
            refs: vec![Ref::Jump(1)]
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
        refs: vec![Ref::Jump(2)]
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

fn refs_of(items: &[Item], prefix: &str) -> Vec<Ref> {
    items
        .iter()
        .find_map(|i| match i {
            Item::Inst { text, refs } if text.starts_with(prefix) => Some(refs.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no instruction {prefix}"))
}

/// The refs of each instruction of routine `f`, which has `params` and `slots`.
fn memory_refs(params: u32, slots: u32, code: Vec<X>) -> Vec<Vec<Ref>> {
    let f = Function {
        name: "f".to_string(),
        params,
        slots,
        body: vec![],
    };
    let m = Module {
        functions: vec![f],
        texts: vec![],
        descs: vec![],
    };
    let mut all = vec![X::Label("f".to_string())];
    all.extend(code);
    let a = Assembly {
        routines: vec![Assembled {
            name: "f".to_string(),
            code: all,
        }],
    };
    let items = listing(&a, &m).unwrap().remove(0).items;
    items
        .into_iter()
        .filter_map(|i| match i {
            Item::Inst { refs, .. } => Some(refs),
            _ => None,
        })
        .collect()
}

fn at(r: R, d: i32) -> Mem {
    Mem::Base(r, d)
}

#[test]
fn slot_and_input_refs() {
    use crate::ir::Reg;
    let items = shaped(
        2,
        3,
        vec![
            Inst::LoadSlot(Reg::A, 2),
            Inst::LoadArg(Reg::A, 1),
            Inst::Return,
        ],
    );
    assert_eq!(refs_of(&items, "mov rax, [rbp-16]"), vec![Ref::Slot(2)]);
    assert_eq!(refs_of(&items, "mov rax, [rbp+24]"), vec![Ref::Input(1)]);
    // The saved `b` word, `[rbp-32]`, is not a slot.
    assert_eq!(refs_of(&items, "mov rcx, [rbp-32]"), vec![]);
}

#[test]
fn slot_and_input_bounds() {
    let load = |m| X::MovRM(R::Rax, m);
    let got = memory_refs(
        2,
        3,
        vec![
            load(at(R::Rbp, -8)),  // first slot
            load(at(R::Rbp, -24)), // last slot
            load(at(R::Rbp, -32)), // saved b
            load(at(R::Rbp, -4)),  // misaligned
            load(at(R::Rbp, 0)),   // saved rbp
            load(at(R::Rbp, 8)),   // return address
            load(at(R::Rbp, 16)),  // input 2
            load(at(R::Rbp, 24)),  // input 1
            load(at(R::Rbp, 32)),  // past the inputs
            load(at(R::Rbp, 20)),  // misaligned
        ],
    );
    let none: Vec<Ref> = vec![];
    assert_eq!(
        got,
        vec![
            vec![Ref::Slot(1)],
            vec![Ref::Slot(3)],
            none.clone(),
            none.clone(),
            none.clone(),
            none.clone(),
            vec![Ref::Input(2)],
            vec![Ref::Input(1)],
            none.clone(),
            none,
        ]
    );
}

#[test]
fn only_rbp_memory_has_slot_and_input_refs() {
    let got = memory_refs(
        1,
        1,
        vec![
            X::MovRM(R::Rax, at(R::Rax, -8)),
            X::MovRM(R::Rax, at(R::Rcx, 16)),
            X::MovRM(R::Rax, at(R::Rsp, -8)),
        ],
    );
    assert_eq!(got, vec![vec![]; 3]);
}

#[test]
fn byte_loads_stores_and_lea_have_refs() {
    let got = memory_refs(
        1,
        1,
        vec![
            X::MovzxRM8(R::Rax, at(R::Rbp, -8)),
            X::MovM8R(at(R::Rbp, 16), R::Rax),
            X::Lea(R::Rax, at(R::Rbp, -8)),
            X::MovMR(at(R::Rbp, -8), R::Rax),
        ],
    );
    assert_eq!(
        got,
        vec![
            vec![Ref::Slot(1)],
            vec![Ref::Input(1)],
            vec![Ref::Slot(1)],
            vec![Ref::Slot(1)]
        ]
    );
}

#[test]
fn jumps_to_other_labels_have_no_jump_ref() {
    let m = module("feature start\n  show 1\n");
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    let mut jumps = 0;
    for i in routines.iter().flat_map(|r| &r.items) {
        if let Item::Inst { text, refs } = i
            && (text.starts_with('j'))
            && !text.contains(".L")
        {
            jumps += 1;
            assert!(refs.is_empty(), "{text}");
        }
    }
    assert!(jumps > 0);
}

#[test]
fn text_and_jump_refs() {
    use crate::ir::{Data, Reg};
    let items = one(vec![
        Inst::Mark(Label(2)),
        Inst::AddressOf(Reg::A, Data::Text(4)),
        Inst::Compare,
        Inst::JumpIf(crate::ir::Cond::Eq, Label(2)),
    ]);
    assert_eq!(refs_of(&items, "lea rax"), vec![Ref::Text(4)]);
    assert_eq!(refs_of(&items, "je "), vec![Ref::Jump(2)]);
}

#[test]
fn every_syscall_is_named() {
    let src = "record Item\n  name as text\n  stock as number\n\ntable inventory of Item in \"inventory.csv\"\n\nfeature start\n  load inventory\n  ask \"Q\" into s\n  show s\n  save inventory\n";
    let m = module(src);
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for i in routines.iter().flat_map(|r| &r.items) {
        if let Item::Inst { text, refs } = i
            && text == "syscall"
        {
            let names: Vec<&str> = refs
                .iter()
                .filter_map(|r| match r {
                    Ref::System(n) => Some(*n),
                    _ => None,
                })
                .collect();
            assert_eq!(names.len(), 1);
            seen.insert(names[0]);
        }
    }
    for n in ["write", "read", "openat2", "renameat", "exit_group"] {
        assert!(seen.contains(n), "{n} not in {seen:?}");
    }
}

#[test]
fn syscall_names_cover_system() {
    use crate::ir::{OpenMode, Prim};
    let prims = [
        Prim::Write,
        Prim::Read,
        Prim::Map,
        Prim::Exit,
        Prim::StackSize,
        Prim::Open(OpenMode::Read),
        Prim::Open(OpenMode::Folder),
        Prim::Open(OpenMode::Create),
        Prim::Open(OpenMode::Probe),
        Prim::Close,
        Prim::Sync,
        Prim::Mode,
        Prim::SetMode,
        Prim::Rename,
        Prim::Fork,
        Prim::Wait,
        Prim::DefaultChildSignal,
    ];
    let want = [
        "write",
        "read",
        "mmap",
        "exit_group",
        "getrlimit",
        "openat2",
        "openat2",
        "openat2",
        "openat2",
        "close",
        "fsync",
        "fstat",
        "fchmod",
        "renameat",
        "fork",
        "wait4",
        "rt_sigaction",
    ];
    let functions = prims
        .iter()
        .enumerate()
        .map(|(i, p)| Function {
            name: format!("p{i}"),
            params: 0,
            slots: 0,
            body: vec![Inst::System(*p)],
        })
        .collect();
    let m = Module {
        functions,
        texts: vec![],
        descs: vec![],
    };
    let routines = listing(&assemble(&m).unwrap(), &m).unwrap();
    for (i, want) in want.iter().enumerate() {
        let r = routines.iter().find(|r| r.name == format!("p{i}")).unwrap();
        let got: Vec<&Ref> = r
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Inst { refs, .. } => Some(refs),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(got, vec![&Ref::System(want)], "{:?}", prims[i]);
    }
}

#[test]
fn an_unnamed_syscall_is_an_error() {
    let m = Module {
        functions: vec![],
        texts: vec![],
        descs: vec![],
    };
    let a = Assembly {
        routines: vec![Assembled {
            name: "f".to_string(),
            code: vec![
                X::Label("f".to_string()),
                X::MovRI(R::Rax, 9999),
                X::Syscall,
            ],
        }],
    };
    let e = listing(&a, &m).unwrap_err();
    assert!(e.contains("has no name") && e.contains('f'), "{e}");
    let a = Assembly {
        routines: vec![Assembled {
            name: "g".to_string(),
            code: vec![X::Label("g".to_string()), X::Syscall],
        }],
    };
    let e = listing(&a, &m).unwrap_err();
    assert!(e.contains("has no name") && e.contains("in g "), "{e}");
}

#[test]
fn a_syscall_does_not_reuse_an_earlier_number() {
    let m = Module {
        functions: vec![],
        texts: vec![],
        descs: vec![],
    };
    let a = Assembly {
        routines: vec![Assembled {
            name: "f".to_string(),
            code: vec![
                X::Label("f".to_string()),
                X::MovRI(R::Rax, 1),
                X::Syscall,
                X::Syscall,
            ],
        }],
    };
    assert!(listing(&a, &m).unwrap_err().contains("has no name"));
}

/// The listing of one routine `f` with `code` after its own label.
fn listed(code: Vec<X>) -> Result<Vec<Item>, String> {
    let mut all = vec![X::Label("f".to_string())];
    all.extend(code);
    let a = Assembly {
        routines: vec![Assembled {
            name: "f".to_string(),
            code: all,
        }],
    };
    let m = Module {
        functions: vec![],
        texts: vec![],
        descs: vec![],
    };
    listing(&a, &m).map(|mut r| r.remove(0).items)
}

#[test]
fn a_syscall_is_named_by_the_mov_that_set_rax_with_pushes_between() {
    let items = listed(vec![X::MovRI(R::Rax, 1), X::Push(R::Rcx), X::Syscall]).unwrap();
    assert!(matches!(&items[2], Item::Inst { refs, .. } if refs == &[Ref::System("write")]));
}

#[test]
fn another_write_to_rax_between_the_mov_and_the_syscall_is_an_error() {
    for between in [
        X::MovRR(R::Rax, R::Rcx),
        X::MovRM(R::Rax, Mem::Base(R::Rbp, -8)),
        X::Pop(R::Rax),
        X::AluI(crate::target::x86_64::inst::AluOp::Add, R::Rax, 1),
        X::Call("g".to_string()),
    ] {
        let err = listed(vec![X::MovRI(R::Rax, 1), between, X::Syscall]).unwrap_err();
        assert!(err.contains("has no name"), "{err}");
    }
}
