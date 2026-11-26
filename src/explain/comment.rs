//! The `;` comment on a listed instruction: what its references mean.

use crate::lower::{FunctionNotes, spell_invisible};
use crate::target::listing::Ref;

/// How many characters of a text a comment shows.
const SHOWN: usize = 30;

/// The comment for an instruction's references, or `None` when no part applies.
pub(super) fn comment(
    refs: &[Ref],
    notes: Option<&FunctionNotes>,
    texts: &[Vec<u8>],
) -> Option<String> {
    let parts: Vec<String> = refs.iter().filter_map(|r| part(r, notes, texts)).collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn part(r: &Ref, notes: Option<&FunctionNotes>, texts: &[Vec<u8>]) -> Option<String> {
    match r {
        Ref::Slot(k) => notes?.slots.get(k).cloned(),
        Ref::Input(k) => notes?.inputs.get((*k as usize).checked_sub(1)?).cloned(),
        Ref::Text(k) => Some(quoted(texts.get(*k as usize)?)),
        Ref::Jump(n) => {
            let fail = notes?.fails.get(n)?;
            Some(format!("failure {}: {}", fail.code, fail.message))
        }
        Ref::System(name) => Some((*name).to_string()),
    }
}

/// The text as a quoted Gloss text, cut after `SHOWN` characters.
fn quoted(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let cut = text.chars().nth(SHOWN).is_some();
    let kept: String = text.chars().take(SHOWN).collect();
    let body = spell_invisible(&kept).replace('"', "\"\"");
    format!("\"{body}{}\"", if cut { "\u{2026}" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::FailNote;

    fn notes() -> FunctionNotes {
        let mut n = FunctionNotes {
            feature: "f".into(),
            line: 1,
            slots: Default::default(),
            inputs: vec!["a".into(), "b".into()],
            fails: Default::default(),
        };
        n.slots.insert(2, "total".into());
        n.fails.insert(
            7,
            FailNote {
                code: 4,
                line: 3,
                message: "boom".into(),
            },
        );
        n
    }

    fn text(s: &str) -> Option<String> {
        comment(&[Ref::Text(0)], None, &[s.as_bytes().to_vec()])
    }

    #[test]
    fn each_reference_kind_has_its_comment() {
        let n = notes();
        let c = |r| comment(&[r], Some(&n), &[]);
        assert_eq!(c(Ref::Slot(2)).as_deref(), Some("total"));
        assert_eq!(c(Ref::Slot(3)), None);
        assert_eq!(c(Ref::Input(2)).as_deref(), Some("b"));
        assert_eq!(c(Ref::Jump(7)).as_deref(), Some("failure 4: boom"));
        assert_eq!(c(Ref::Jump(8)), None);
        assert_eq!(c(Ref::System("write")).as_deref(), Some("write"));
        assert_eq!(comment(&[Ref::Slot(2)], None, &[]), None);
    }

    #[test]
    fn parts_are_joined_with_a_comma() {
        let n = notes();
        let got = comment(&[Ref::Slot(2), Ref::Input(1)], Some(&n), &[]);
        assert_eq!(got.as_deref(), Some("total, a"));
    }

    #[test]
    fn a_text_is_cut_after_thirty_characters() {
        let thirty = "a".repeat(30);
        assert_eq!(text(&thirty), Some(format!("\"{thirty}\"")));
        let more = "a".repeat(31);
        assert_eq!(text(&more), Some(format!("\"{thirty}\u{2026}\"")));
    }

    #[test]
    fn quotes_double_and_invisible_characters_are_spelled() {
        assert_eq!(text("say \"hi\"").as_deref(), Some("\"say \"\"hi\"\"\""));
        assert_eq!(text("a\u{202E}b").as_deref(), Some("\"a\\u{202E}b\""));
        assert_eq!(text("\n").as_deref(), Some("\"\\u{000A}\""));
    }

    #[test]
    fn invalid_utf8_decodes_lossily() {
        let got = comment(&[Ref::Text(0)], None, &[vec![b'a', 0xFF]]);
        assert_eq!(got.as_deref(), Some("\"a\u{FFFD}\""));
    }

    #[test]
    fn five_digit_escapes_and_all_invisible_text() {
        assert_eq!(text("\u{E0001}").as_deref(), Some("\"\\u{E0001}\""));
        let many = "\u{200B}".repeat(31);
        let want = format!("\"{}\u{2026}\"", "\\u{200B}".repeat(30));
        assert_eq!(text(&many), Some(want));
    }

    #[test]
    fn out_of_range_references_have_no_comment() {
        let n = notes();
        assert_eq!(comment(&[Ref::Input(0)], Some(&n), &[]), None);
        assert_eq!(comment(&[Ref::Input(3)], Some(&n), &[]), None);
        assert_eq!(comment(&[Ref::Text(1)], None, &[b"a".to_vec()]), None);
    }
}
