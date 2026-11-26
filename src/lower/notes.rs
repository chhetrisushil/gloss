//! What lowering knows that `gloss explain` shows: each feature's slot and input names.

use std::collections::BTreeMap;

/// Notes for a whole program, keyed by IR function name (`f_<feature>`).
#[derive(Default, Debug, PartialEq)]
pub struct Notes {
    pub functions: BTreeMap<String, FunctionNotes>,
}

#[derive(Default, Debug, PartialEq)]
pub struct FunctionNotes {
    pub feature: String,
    /// The line of the feature's header.
    pub line: u32,
    /// Named slots only; a hidden temporary has no name.
    pub slots: BTreeMap<u32, String>,
    /// In order; input k is `inputs[k - 1]`.
    pub inputs: Vec<String>,
    /// Filled in by the failure notes (a later task).
    pub fails: BTreeMap<u32, FailNote>,
}

#[derive(Debug, PartialEq)]
pub struct FailNote {
    pub code: i64,
    pub line: u32,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use crate::check::check;
    use crate::ir::module_text;
    use crate::lower::{Mode, lower_explained, lower_program};
    use crate::source::Source;

    fn explained(text: &str) -> (crate::ir::Module, super::Notes) {
        let src = Source::new(text);
        let program = crate::parse_source(&src).expect("parses");
        let types = check(&program, &src).types.expect("checks clean");
        lower_explained(&program, &types, &src).expect("lowers")
    }

    #[test]
    fn notes_name_every_slot_and_input() {
        let (_, notes) = explained(
            "feature total_of\n  needs prices as list of number\n  set total to 0\n  \
             for each price in prices\n    add price to total\n  give total\n\n\
             feature start\n  show (total_of with (list of 1, 2, 3))\n",
        );
        let f = &notes.functions["f_total_of"];
        assert_eq!(f.feature, "total_of");
        assert_eq!(f.line, 1);
        assert_eq!(
            f.slots,
            [(1, "total".to_string()), (2, "price".to_string())].into()
        );
        assert_eq!(f.inputs, ["prices"]);
        assert!(f.fails.is_empty());
        let s = &notes.functions["f_start"];
        assert_eq!((s.line, s.feature.as_str()), (8, "start"));
        assert!(s.inputs.is_empty());
    }

    #[test]
    fn taking_notes_does_not_change_the_module() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "gloss") {
                continue;
            }
            let src = Source::new(&std::fs::read_to_string(&path).unwrap());
            let program = crate::parse_source(&src).expect("parses");
            let types = check(&program, &src).types.expect("checks clean");
            let plain = lower_program(&program, &types, &src, Mode::Real, None).unwrap();
            let (with, _) = lower_explained(&program, &types, &src).unwrap();
            assert_eq!(module_text(&plain), module_text(&with), "{path:?}");
            seen += 1;
        }
        assert!(seen >= 12, "only {seen} examples");
    }
}
