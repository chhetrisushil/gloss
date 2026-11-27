//! `gloss flowchart`: a feature's statements as a graph, then a layout and SVG text (§4.11).

#[cfg(test)]
mod check;
mod graph;
mod layout;
mod route;
mod svg;
#[cfg(test)]
mod test_support;

use crate::ast::program::Program;
use crate::check::TypeInfo;
use crate::source::Source;

/// One `(file name, svg)` pair per feature, in source order; the stem is `name` without `.gloss`.
pub(crate) fn charts(
    program: &Program,
    src: &Source,
    types: &TypeInfo,
    name: &str,
) -> Vec<(String, String)> {
    let stem = name.strip_suffix(".gloss").unwrap_or(name);
    program
        .features()
        .map(|f| {
            let (g, title) = (
                graph::graph(f, src, types),
                format!("{name}: feature {}", f.name),
            );
            let svg = svg::svg(&title, &g, &layout::layout(&g));
            (format!("{stem}.{}.svg", f.name), svg)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn charts_are_named_by_stem_and_feature_in_source_order() {
        let text = "feature start\n  show 1\n\nfeature other\n  show 2\n";
        let (_, checked) = crate::check_program(text);
        let checked = checked.unwrap();
        let src = crate::source::Source::new(text);
        let out = super::charts(checked.program(), &src, &checked.types, "x.gloss");
        let names: Vec<&str> = out.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(names, ["x.start.svg", "x.other.svg"]);
        assert!(out[1].1.contains("<title>x.gloss: feature other</title>\n"));
    }

    #[test]
    fn a_feature_ending_in_a_continued_statement_ends_on_its_last_line() {
        let text = "feature start\n  show 1,\n    2\n";
        let (_, checked) = crate::check_program(text);
        let checked = checked.unwrap();
        let feature = checked.program().features().next().unwrap();
        let end = |src: &crate::source::Source| {
            let g = super::graph::graph(feature, src, &checked.types);
            g.nodes.last().unwrap().line
        };
        assert_eq!(end(&checked.written), 3);
        assert_eq!(end(&checked.src), 2, "the joined source ends it early");
    }
}
