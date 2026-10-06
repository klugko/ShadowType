use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub wpm: f64, pub raw_wpm: f64, pub accuracy: f64,
    pub errors: usize, pub correct: usize, pub attempts: usize,
    pub position: usize, pub length: usize, pub elapsed: f64,
}
pub struct Session {
    pub target: Vec<String>, pub typed: String,
    pub started: Option<Instant>, pub finished: Option<Instant>,
    pub limit: Option<Duration>, attempts: usize, correct_attempts: usize,
}
impl Session {
    pub fn new(text: &str, limit: Option<Duration>) -> Self {
        Self { target: text.graphemes(true).map(str::to_owned).collect(), typed: String::new(), started: None, finished: None, limit, attempts: 0, correct_attempts: 0 }
    }
    pub fn input(&mut self, ch: char) {
        if self.done() { return; }
        self.started.get_or_insert_with(Instant::now);
        self.typed.push(ch);
        let typed: Vec<_> = self.typed.graphemes(true).collect();
        self.attempts += 1;
        let i = typed.len().saturating_sub(1);
        if self.target.get(i).is_some_and(|t| t.starts_with(typed[i])) { self.correct_attempts += 1; }
        if typed.len() == self.target.len() && !self.target.get(i).is_some_and(|t| t != typed[i] && t.starts_with(typed[i])) { self.finished = Some(Instant::now()); }
    }
    pub fn backspace(&mut self) {
        if self.done() { return; }
        if let Some((i,_)) = self.typed.grapheme_indices(true).next_back() { self.typed.truncate(i); }
    }
    pub fn elapsed(&self) -> Duration {
        self.started.map(|start| self.finished.unwrap_or_else(Instant::now).saturating_duration_since(start)).unwrap_or_default()
    }
    pub fn tick(&mut self) {
        if let (Some(start),Some(limit)) = (self.started,self.limit) {
            if self.finished.is_none() && start.elapsed() >= limit { self.finished = Some(start + limit); }
        }
    }
    pub fn done(&self) -> bool { self.finished.is_some() }
    pub fn stats(&self) -> Stats {
        let typed: Vec<_> = self.typed.graphemes(true).collect();
        let correct = typed.iter().zip(&self.target).filter(|(a,b)| **a == b.as_str()).count();
        let elapsed = self.elapsed().as_secs_f64();
        Stats { wpm: speed(correct,elapsed), raw_wpm: speed(self.attempts,elapsed), accuracy: if self.attempts == 0 {100.0} else {self.correct_attempts as f64 / self.attempts as f64 * 100.0}, errors: self.attempts - self.correct_attempts, correct, attempts: self.attempts, position: typed.len(), length: self.target.len(), elapsed }
    }
}
pub fn speed(chars: usize, seconds: f64) -> f64 {
    if seconds <= 0.0 {0.0} else { chars as f64 * 12.0 / seconds }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn wpm_and_zero() { assert_eq!(speed(300,60.0),60.0); assert_eq!(speed(5,0.0),0.0); }
    #[test] fn mistakes_survive_correction() {
        let mut s = Session::new("ab",None);
        s.input('x'); s.backspace(); s.input('a'); s.input('b');
        let stats=s.stats(); assert_eq!(stats.errors,1); assert_eq!(stats.correct,2);
        assert!((stats.accuracy - 200.0/3.0).abs()<0.001); assert!(s.done());
        s.input('c'); assert_eq!(s.typed,"ab");
    }
    #[test] fn unicode_and_backspace() {
        let mut s = Session::new("é👩‍💻x",None);
        for ch in "é👩‍💻".chars() {s.input(ch);}
        assert_eq!(s.stats().position,2); s.backspace(); assert_eq!(s.typed,"é");
    }
    #[test] fn final_combined_grapheme() {
        let mut s=Session::new("e\u{301}",None);
        s.input('e'); assert!(!s.done()); s.input('\u{301}');
        assert!(s.done()); assert_eq!(s.stats().accuracy,100.0);
        let mut emoji=Session::new("👩‍💻",None);
        for ch in "👩‍💻".chars() {emoji.input(ch);}
        assert!(emoji.done());assert_eq!(emoji.stats().correct,1);
    }
    #[test] fn timer_freezes() {
        let mut s=Session::new("abc",Some(Duration::from_millis(1)));
        s.input('a'); std::thread::sleep(Duration::from_millis(3)); s.tick();
        assert!(s.done()); assert_eq!(s.elapsed(),Duration::from_millis(1));
    }
}
