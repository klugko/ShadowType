use std::time::{Duration, Instant};

use code_racer_engine::{
    Sample, SessionOptions, Stats, Status, TypingSession, WordStream, consistency,
};

use super::Plan;
use crate::{
    app::{ink::Ink, text_event::TextEvent},
    history::{History, Record},
};

const TIMED_INITIAL_WORDS: usize = 80;
const TIMED_REFILL_WORDS: usize = 40;
const TIMED_REFILL_BELOW: usize = 120;
/// Most missed characters the results list.
const MISSED_SHOWN: usize = 5;

/// A solo session in progress or just finished.
#[derive(Debug)]
pub struct SoloRun {
    pub plan: Plan,
    session: TypingSession,
    ink: Ink,
    pub attribution: Option<String>,
    pub result: Option<SoloResult>,
    feed: Option<WordStream>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoloResult {
    pub stats: Stats,
    pub samples: Vec<Sample>,
    pub consistency: f64,
    pub previous_best: Option<f64>,
    /// Average speed of the recent sessions before this one, if any.
    pub recent_wpm: Option<f64>,
    /// The characters missed most, with how many times.
    pub missed: Vec<(String, u32)>,
    /// When the session ended and the results came on screen.
    pub at: Instant,
}

impl SoloResult {
    pub fn is_personal_best(&self) -> bool {
        self.previous_best.is_none_or(|best| self.stats.wpm > best)
    }
}

impl SoloRun {
    pub fn start(plan: Plan, seed: u64) -> Self {
        let code_like = plan.syntax().is_some() || matches!(plan, Plan::File(_));
        let auto_indent = SessionOptions {
            auto_indent: code_like,
            ..SessionOptions::default()
        };
        let (session, attribution, feed) = match &plan {
            Plan::Text(source) => {
                let generated = source.generate(seed);
                let session = TypingSession::new(&generated.text, auto_indent);
                (session, generated.attribution, None)
            }
            Plan::Timed {
                language,
                options,
                seconds,
            } => {
                let mut feed = WordStream::new(*language, *options, seed);
                let limit = SessionOptions {
                    time_limit: Some(Duration::from_secs(u64::from(*seconds))),
                    auto_indent: false,
                };
                let session = TypingSession::new(&feed.phrase(TIMED_INITIAL_WORDS), limit);
                (session, None, Some(feed))
            }
            Plan::File(custom) => (TypingSession::new(&custom.text, auto_indent), None, None),
        };
        Self {
            plan,
            session,
            ink: Ink::default(),
            attribution,
            result: None,
            feed,
        }
    }

    pub fn session(&self) -> &TypingSession {
        &self.session
    }

    pub fn ink(&self) -> &Ink {
        &self.ink
    }

    /**
     * Hands `event` to the session, refilling a timed text that runs low.
     * Returns whether the session took the key.
     */
    pub fn text_event(&mut self, event: TextEvent, now: Instant) -> bool {
        let taken = self.ink.apply(event, &mut self.session, now);
        if taken {
            self.refill();
        }
        taken
    }

    pub fn is_finished(&self) -> bool {
        self.session.is_finished()
    }

    /// Whether typing has started and is not over.
    pub fn is_in_progress(&self) -> bool {
        self.session.status() == Status::Running
    }

    /**
     * Computes the result once the session is over, compared with the
     * best of `history`, and returns the record to keep, only once.
     */
    pub fn conclude(&mut self, history: &History, now: Instant) -> Option<Record> {
        if self.result.is_some() || !self.session.is_finished() {
            return None;
        }
        let stats = self.session.stats(now);
        let samples = self.session.samples(now);
        let mode = self.plan.mode_label();
        let language = self.plan.language_label();
        let summary = history.summary();
        let result = SoloResult {
            consistency: consistency(&samples),
            previous_best: history.personal_best(&mode, &language),
            recent_wpm: (summary.sessions > 0).then_some(summary.recent_wpm),
            missed: self.ink.most_missed(MISSED_SHOWN),
            at: now,
            stats,
            samples,
        };
        self.result = Some(result);
        Some(Record::from_stats(mode, language, &stats))
    }

    fn refill(&mut self) {
        let Some(feed) = &mut self.feed else {
            return;
        };
        if self.session.remaining() < TIMED_REFILL_BELOW {
            self.session
                .extend(&format!(" {}", feed.phrase(TIMED_REFILL_WORDS)));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use code_racer_engine::{CodeLanguage, Language, Mark, TextSource};

    use super::*;
    use crate::{
        config::{Mode, Practice},
        persist::scratch::TempDir,
    };

    fn type_all(run: &mut SoloRun, now: Instant) {
        let text = run.session.target().concat();
        for ch in text.chars() {
            run.text_event(TextEvent::Typed(ch), now);
        }
    }

    #[test]
    fn words_session_has_the_requested_length() {
        let settings = Practice {
            mode: Mode::Words,
            word_count: 10,
            ..Practice::default()
        };
        let run = SoloRun::start(Plan::from_practice(&settings), 3);
        let text = run.session.target().concat();
        assert_eq!(text.split(' ').count(), 10);
    }

    #[test]
    fn timed_session_never_runs_out_of_text() {
        let settings = Practice {
            mode: Mode::Time,
            duration: 15,
            ..Practice::default()
        };
        let mut run = SoloRun::start(Plan::from_practice(&settings), 5);
        let now = Instant::now();
        for _ in 0..3_000 {
            let next = run.session.target()[run.session.cursor()].clone();
            for ch in next.chars() {
                run.text_event(TextEvent::Typed(ch), now);
            }
        }
        assert!(run.session.remaining() >= TIMED_REFILL_BELOW / 2);
        assert_eq!(run.session.status(), Status::Running);
    }

    #[test]
    fn a_code_session_is_compared_with_the_code_records_of_version_one() {
        let directory = TempDir::new();
        let path = directory.join("history.json");
        let legacy = r#"[{"date":"2025-05-01T10:00:00+00:00","mode":"code/50","language":"rust",
            "stats":{"wpm":62.5,"raw_wpm":70.0,"accuracy":96.0,"errors":4,"length":250,"elapsed":48.0}}]"#;
        fs::write(&path, legacy).expect("write");
        let mut history = History::load(&path).value;
        let rust = TextSource::Code {
            language: CodeLanguage::Rust,
        };
        let mut run = SoloRun::start(Plan::Text(rust), 1);
        let now = Instant::now();
        run.session.start(now);
        while !run.is_finished() {
            let before = run.session.cursor();
            let next = run.session.target()[before].clone();
            for ch in next.chars() {
                let typed = run.text_event(TextEvent::Typed(ch), now + Duration::from_secs(60));
                assert!(typed, "{ch:?} refused at {before}");
            }
            assert_eq!(
                run.session.mark(before),
                Mark::Correct,
                "{next:?} at {before}"
            );
        }
        let record = run
            .conclude(&history, now + Duration::from_secs(60))
            .expect("a record");
        history.add(record).expect("save");
        let result = run.result.as_ref().expect("result");
        assert_eq!(result.previous_best, Some(62.5));
        let records = history.records();
        assert_eq!(records[1].mode, records[0].mode);
    }

    #[test]
    fn code_sessions_auto_indent() {
        let run = SoloRun::start(
            Plan::Text(TextSource::Code {
                language: CodeLanguage::Rust,
            }),
            1,
        );
        assert!(run.session.options().auto_indent);
    }

    #[test]
    fn conclusion_is_recorded_once_with_personal_best() {
        let mut history = History::in_memory();
        let now = Instant::now();
        let mut run = SoloRun::start(
            Plan::Text(TextSource::Quote {
                language: Language::English,
            }),
            2,
        );
        run.session.start(now);
        type_all(&mut run, now + Duration::from_secs(30));
        assert!(run.is_finished());
        let record = run
            .conclude(&history, now + Duration::from_secs(30))
            .expect("a record");
        history.add(record).expect("in-memory history");
        assert_eq!(
            run.conclude(&history, now + Duration::from_secs(31)),
            None,
            "only once"
        );
        assert_eq!(history.records().len(), 1);
        let result = run.result.as_ref().expect("result");
        assert!(result.is_personal_best());
        assert_eq!(history.records()[0].mode, "quote");
        assert_eq!(history.records()[0].language, "english");
    }
}
