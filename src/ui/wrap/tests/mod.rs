mod invariants;
mod layout;
mod locate;

use code_racer_engine::graphemes;

use super::*;

fn rows(text: &str, width: u16) -> Vec<String> {
    let graphemes = graphemes(text);
    wrap(&graphemes, width)
        .iter()
        .map(|line| graphemes[line.start..line.end].concat())
        .collect()
}

fn numbers(text: &str, width: u16) -> Vec<Option<usize>> {
    wrap(&graphemes(text), width)
        .iter()
        .map(|line| line.number)
        .collect()
}

fn position(text: &str, width: u16, index: usize) -> (usize, usize) {
    let graphemes = graphemes(text);
    locate(&graphemes, &wrap(&graphemes, width), index)
}
