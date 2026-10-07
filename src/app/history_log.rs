//! Layout of the `history.log` buffer: what each of its lines shows. The
//! view draws these lines, and the keys that scroll it stop on the last one.

/// Sessions shown in the progression chart, the most recent ones.
pub const CHART_SESSIONS: usize = 60;
/// Rows of the progression chart.
pub const CHART_HEIGHT: u16 = 6;

/// A figure of the summary of the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    BestWpm,
    AverageWpm,
    RecentWpm,
    BestAccuracy,
}

impl Figure {
    pub const ALL: [Self; 4] = [
        Self::BestWpm,
        Self::AverageWpm,
        Self::RecentWpm,
        Self::BestAccuracy,
    ];
}

/// What a line of `history.log` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Title,
    Blank,
    /// The hint of a history without sessions.
    NoSessions,
    /// How many sessions there are, and the time spent on them.
    Totals,
    Figure(Figure),
    /// The heading of the chart of the last `sessions` sessions.
    ChartHeading {
        sessions: usize,
    },
    /// A row of the progression chart, from the top.
    Chart(u16),
    /// The names of the columns of the sessions.
    Columns,
    /// A session, by its rank from the most recent, which comes first.
    Session(usize),
}

/// The lines of the buffer for a history of `sessions` sessions. The chart
/// needs two sessions to draw a line.
pub fn lines(sessions: usize) -> Vec<Line> {
    if sessions == 0 {
        return vec![Line::Title, Line::Blank, Line::NoSessions];
    }
    let mut lines = vec![Line::Title, Line::Totals, Line::Blank];
    lines.extend(Figure::ALL.map(Line::Figure));
    let charted = sessions.min(CHART_SESSIONS);
    if charted >= 2 {
        lines.extend([Line::Blank, Line::ChartHeading { sessions: charted }]);
        lines.extend((0..CHART_HEIGHT).map(Line::Chart));
    }
    lines.extend([Line::Blank, Line::Columns]);
    lines.extend((0..sessions).map(Line::Session));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_history_only_says_how_to_start() {
        assert_eq!(lines(0), [Line::Title, Line::Blank, Line::NoSessions]);
    }

    #[test]
    fn the_chart_needs_two_sessions() {
        assert!(!lines(1).contains(&Line::Chart(0)));
        let two = lines(2);
        assert!(two.contains(&Line::ChartHeading { sessions: 2 }));
        assert!(two.contains(&Line::Chart(CHART_HEIGHT - 1)));
        assert!(!two.contains(&Line::Chart(CHART_HEIGHT)));
    }

    #[test]
    fn every_session_follows_the_summary_and_the_chart() {
        let sessions = CHART_SESSIONS + 1;
        let lines = lines(sessions);
        assert!(lines.contains(&Line::ChartHeading {
            sessions: CHART_SESSIONS
        }));
        let columns = lines
            .iter()
            .position(|line| *line == Line::Columns)
            .expect("the column names");
        let ranks: Vec<Line> = (0..sessions).map(Line::Session).collect();
        assert_eq!(lines[columns + 1..], ranks);
        assert_eq!(
            lines
                .iter()
                .filter(|line| matches!(line, Line::Figure(_)))
                .count(),
            Figure::ALL.len()
        );
    }
}
