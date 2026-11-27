//! Writing a layout as SVG text, one element per line (§4.11).

use super::graph::{Graph, Shape};
use super::layout::Layout;
use std::fmt::Write;

/// Replaces the four characters XML text and attributes cannot hold as they are.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn svg(title: &str, g: &Graph, l: &Layout) -> String {
    let (w, h) = (l.width, l.height);
    let mut o = String::new();
    let _ = write!(
        o,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"monospace\" font-size=\"15\">\n\
<title>{}</title>\n\
<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto\"><path d=\"M0,0 L10,5 L0,10 z\"/></marker></defs>\n\
<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n",
        escape(title)
    );
    for (node, r) in g.nodes.iter().zip(&l.boxes) {
        let (x, y, w, h) = (r.x, r.y, r.w, r.h);
        let mut text_x = x + 12;
        match node.shape {
            Shape::Decision => {
                let (cy, e) = (y + h / 2, h / 2);
                text_x += e;
                let _ = writeln!(
                    o,
                    "<polygon points=\"{x},{cy} {},{y} {},{y} {},{cy} {},{} {},{}\" fill=\"white\" stroke=\"black\"/>",
                    x + e,
                    x + w - e,
                    x + w,
                    x + w - e,
                    y + h,
                    x + e,
                    y + h
                );
            }
            shape => {
                let round = matches!(shape, Shape::Start | Shape::End);
                let rx = if round { " rx=\"12\"" } else { "" };
                let _ = writeln!(
                    o,
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"{rx} fill=\"white\" stroke=\"black\"/>"
                );
                if shape == Shape::Call {
                    for lx in [x + 6, x + w - 6] {
                        let _ = writeln!(
                            o,
                            "<line x1=\"{lx}\" y1=\"{y}\" x2=\"{lx}\" y2=\"{}\" stroke=\"black\"/>",
                            y + h
                        );
                    }
                }
            }
        }
        for (i, line) in node.label.iter().enumerate() {
            let _ = writeln!(
                o,
                "<text x=\"{text_x}\" y=\"{}\">{}</text>",
                y + 23 + 20 * i as i32,
                escape(line)
            );
        }
    }
    for (a, route) in g.arrows.iter().zip(&l.routes) {
        let points: Vec<String> = route.iter().map(|(x, y)| format!("{x},{y}")).collect();
        let _ = writeln!(
            o,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>",
            points.join(" ")
        );
        if let (Some(word), Some(&(x0, y0))) = (a.kind.label(), route.first()) {
            let _ = writeln!(
                o,
                "<text x=\"{}\" y=\"{}\" font-size=\"12\">{word}</text>",
                x0 + 4,
                y0 + 12
            );
        }
    }
    o.push_str("</svg>\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(text: &str) -> String {
        super::super::check::program(text);
        let (_, checked) = crate::check_program(text);
        let checked = checked.expect("the program checks");
        let f = checked.program().features().next().unwrap();
        let g = super::super::graph::graph(f, &crate::source::Source::new(text));
        svg(
            "x.gloss: feature start",
            &g,
            &super::super::layout::layout(&g),
        )
    }

    #[test]
    fn escape_replaces_the_four_characters() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn a_one_statement_chart() {
        let want = "\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"213\" height=\"188\" viewBox=\"0 0 213 188\" font-family=\"monospace\" font-size=\"15\">
<title>x.gloss: feature start</title>
<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto\"><path d=\"M0,0 L10,5 L0,10 z\"/></marker></defs>
<rect width=\"100%\" height=\"100%\" fill=\"white\"/>
<rect x=\"16\" y=\"16\" width=\"141\" height=\"36\" rx=\"12\" fill=\"white\" stroke=\"black\"/>
<text x=\"28\" y=\"39\">feature start</text>
<rect x=\"47\" y=\"76\" width=\"78\" height=\"36\" fill=\"white\" stroke=\"black\"/>
<text x=\"59\" y=\"99\">show 1</text>
<rect x=\"61\" y=\"136\" width=\"51\" height=\"36\" rx=\"12\" fill=\"white\" stroke=\"black\"/>
<text x=\"73\" y=\"159\">end</text>
<polyline points=\"86,52 86,76\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
<polyline points=\"86,112 86,136\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
</svg>
";
        assert_eq!(chart("feature start\n  show 1\n"), want);
    }

    #[test]
    fn labels_are_escaped() {
        let s = chart("feature start\n  show \"a<b & \"\"c\"\"\"\n");
        assert!(
            s.contains(">show &quot;a&lt;b &amp; &quot;&quot;c&quot;&quot;&quot;</text>"),
            "{s}"
        );
    }

    #[test]
    fn a_decision_and_a_call_chart() {
        let want = "\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"253\" height=\"248\" viewBox=\"0 0 253 248\" font-family=\"monospace\" font-size=\"15\">
<title>x.gloss: feature start</title>
<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto\"><path d=\"M0,0 L10,5 L0,10 z\"/></marker></defs>
<rect width=\"100%\" height=\"100%\" fill=\"white\"/>
<rect x=\"16\" y=\"16\" width=\"141\" height=\"36\" rx=\"12\" fill=\"white\" stroke=\"black\"/>
<text x=\"28\" y=\"39\">feature start</text>
<polygon points=\"16,94 34,76 139,76 157,94 139,112 34,112\" fill=\"white\" stroke=\"black\"/>
<text x=\"46\" y=\"99\">if 1 is 1</text>
<rect x=\"29\" y=\"136\" width=\"114\" height=\"36\" fill=\"white\" stroke=\"black\"/>
<line x1=\"35\" y1=\"136\" x2=\"35\" y2=\"172\" stroke=\"black\"/>
<line x1=\"137\" y1=\"136\" x2=\"137\" y2=\"172\" stroke=\"black\"/>
<text x=\"41\" y=\"159\">run helper</text>
<rect x=\"61\" y=\"196\" width=\"51\" height=\"36\" rx=\"12\" fill=\"white\" stroke=\"black\"/>
<text x=\"73\" y=\"219\">end</text>
<polyline points=\"86,52 86,76\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
<polyline points=\"86,112 86,136\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
<text x=\"90\" y=\"124\" font-size=\"12\">yes</text>
<polyline points=\"86,172 86,196\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
<polyline points=\"157,94 197,94 197,184 86,184 86,196\" fill=\"none\" stroke=\"black\" marker-end=\"url(#arrow)\"/>
<text x=\"161\" y=\"106\" font-size=\"12\">no</text>
</svg>
";
        let text = "feature start\n  if 1 is 1\n    run helper\n\nfeature helper\n  show 2\n";
        assert_eq!(chart(text), want);
    }
}
