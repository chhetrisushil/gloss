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
    /// By label number: what each failure block reports.
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
        let fails: Vec<_> = f.fails.values().map(|n| (n.code, n.line)).collect();
        assert_eq!(fails, [(4, 5)]);
        assert_eq!(
            f.fails.values().next().unwrap().message,
            "a number went out of range (add price to total)"
        );
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

    #[test]
    fn failure_notes_say_the_message() {
        let (_, notes) = explained(
            "feature start\n  set xs to list of 1, 2\n  set n to 3\n  set t to \"x\"\n  \
             set a to 9 / n\n  show (item n of xs)\n  show (t as number)\n  \
             show n * 1000000000000\n",
        );
        let got: Vec<_> = notes.functions["f_start"]
            .fails
            .values()
            .map(|f| (f.code, f.line, f.message.as_str()))
            .collect();
        let want = [
            (1, 5, "dividing by zero (n is 0)"),
            (4, 5, "a number went out of range (9 / n)"),
            (3, 6, "an item position must be whole (n is \u{2026})"),
            (2, 6, "there is no item \u{2026} in xs; it has \u{2026}"),
            (5, 7, "\"\u{2026}\" is not a number (t as number)"),
            (4, 8, "a number went out of range (n * 1000000000000)"),
        ];
        assert_eq!(got, want);
    }
}
