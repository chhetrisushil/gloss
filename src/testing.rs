/// The complete example in section 13 of the language spec.
pub fn expenses() -> String {
    let spec = include_str!("../spec/v0.md");
    let after = spec
        .split("## 13. Complete example")
        .nth(1)
        .expect("spec §13 heading");
    let fenced = after
        .split("```gloss\n")
        .nth(1)
        .expect("spec §13 gloss fence");
    fenced.split("```").next().unwrap().to_string()
}
