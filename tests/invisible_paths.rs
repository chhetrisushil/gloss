//! A table file name cannot hide characters that would not show in the permission list.

use gloss::check_source;

fn errors(path: &str) -> Vec<String> {
    let text = format!(
        "record E\n  a as text\ntable es of E in \"{path}\"\nfeature start\n  load es\n  show es\n"
    );
    check_source(&text)
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.problem.clone())
        .collect()
}

#[test]
fn invisible_and_space_like_characters_are_rejected() {
    let hidden = [
        '\u{200E}',
        '\u{200F}',
        '\u{00AD}',
        '\u{061C}',
        '\u{FE0F}',
        '\u{2063}',
        '\u{00A0}',
        '\u{3000}',
        '\u{034F}',
        '\u{115F}',
        '\u{17B4}',
        '\u{180B}',
        '\u{3164}',
        '\u{FFA0}',
        '\u{FFF0}',
        '\u{1BCA0}',
        '\u{1D173}',
        '\u{E0001}',
        '\u{E0100}',
        '\u{1680}',
        '\u{2000}',
        '\u{200A}',
        '\u{202F}',
        '\u{205F}',
        '\u{FEFF}',
        '\u{2066}',
    ];
    for c in hidden {
        let found = errors(&format!("a{c}b.csv"));
        assert_eq!(found.len(), 1, "U+{:04X}: {found:?}", c as u32);
        assert!(
            found[0].contains("a character that cannot be shown"),
            "{found:?}"
        );
    }
}

#[test]
fn an_ordinary_non_ascii_name_is_accepted() {
    assert!(errors("données.csv").is_empty());
    assert!(errors("日本語/表.csv").is_empty());
}
