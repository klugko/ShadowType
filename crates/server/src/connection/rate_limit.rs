use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const RATE_LIMIT: usize = 40;
const RATE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub(super) struct RateLimiter {
    arrivals: VecDeque<Instant>,
}

impl RateLimiter {
    pub(super) fn allow(&mut self, now: Instant) -> bool {
        while self
            .arrivals
            .front()
            .is_some_and(|&arrival| now.saturating_duration_since(arrival) >= RATE_WINDOW)
        {
            self.arrivals.pop_front();
        }
        self.arrivals.push_back(now);
        self.arrivals.len() <= RATE_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burst(limiter: &mut RateLimiter, count: usize, at: Instant) -> Vec<bool> {
        (0..count).map(|_| limiter.allow(at)).collect()
    }

    #[test]
    fn allows_forty_messages_within_a_second_but_not_forty_one() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        assert!(
            burst(&mut limiter, RATE_LIMIT, start)
                .iter()
                .all(|&allowed| allowed)
        );
        assert!(!limiter.allow(start + Duration::from_millis(999)));
    }

    #[test]
    fn the_window_slides_with_time() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        for index in 0..200u64 {
            assert!(
                limiter.allow(start + Duration::from_millis(index * 25)),
                "message {index} arrives at 40 per second"
            );
        }
    }

    #[test]
    fn old_messages_stop_counting_after_a_second() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        burst(&mut limiter, RATE_LIMIT, start);
        let later = start + RATE_WINDOW;
        assert!(
            burst(&mut limiter, RATE_LIMIT, later)
                .iter()
                .all(|&allowed| allowed)
        );
        assert!(!limiter.allow(later));
    }
}
