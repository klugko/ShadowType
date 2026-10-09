use super::Entry;

/// An entry that matches the query, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub entry: Entry,
    /// Indexes of the characters of the title that match the query.
    pub positions: Vec<usize>,
}

/**
 * The entries matching `query`, best first. Every entry matches an
 * empty query, in its own order.
 */
pub fn matches(entries: Vec<Entry>, query: &str) -> Vec<Match> {
    let mut found: Vec<(i64, Match)> = entries
        .into_iter()
        .filter_map(|entry| {
            let (score, positions) = fuzzy(&entry.title, query)?;
            Some((score, Match { entry, positions }))
        })
        .collect();
    found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    found.into_iter().map(|(_, found)| found).collect()
}

/**
 * How well `query` matches `title`, and the positions of the characters
 * it matched. Its letters match in order and in any case, each one at the
 * start of a word or further in the word of the letter before it, so that
 * `cod` finds `Code` but not the `c`, `o` and `d` of `seconds`. Letters
 * that follow each other or start a word score higher.
 */
fn fuzzy(title: &str, query: &str) -> Option<(i64, Vec<usize>)> {
    let query: Vec<char> = query
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    let title: Vec<char> = title.chars().collect();
    let starts_word = |index: usize| index == 0 || !title[index - 1].is_alphanumeric();
    let mut positions: Vec<usize> = Vec::with_capacity(query.len());
    let mut score = 0;
    for wanted in query {
        let from = positions.last().map_or(0, |last| last + 1);
        let mut same_word = !positions.is_empty();
        let found = (from..title.len()).find(|&index| {
            if index > from && !title[index - 1].is_alphanumeric() {
                same_word = false;
            }
            let matches = title[index].to_lowercase().eq(std::iter::once(wanted));
            matches && (same_word || starts_word(index))
        })?;
        let follows = positions.last().is_some_and(|last| last + 1 == found);
        score += 1 + i64::from(starts_word(found)) * 8 + i64::from(follows) * 5;
        score -= i64::try_from(found - from).unwrap_or(0).min(3);
        positions.push(found);
    }
    Some((score, positions))
}
