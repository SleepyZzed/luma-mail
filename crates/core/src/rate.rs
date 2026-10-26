//! A per-account rolling budget below Gmail’s 6,000 units/minute limit.
//! Allows ordinary page/folder browsing to burst, without artificial per-page waits.
use std::collections::VecDeque;
use tokio::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub(crate) struct Rate {
    state: Mutex<Budget>,
}
struct Budget {
    charges: VecDeque<(Instant, u32)>,
    used: u32,
}
const LIMIT: u32 = 5500;
const WINDOW: Duration = Duration::from_secs(60);
impl Rate {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(Budget {
                charges: VecDeque::new(),
                used: 0,
            }),
        }
    }
    pub(crate) async fn acquire(&self, units: u32) {
        assert!(units <= LIMIT, "Gmail request exceeds minute budget");
        loop {
            let wait = {
                let mut budget = self.state.lock().await;
                let now = Instant::now();
                while budget
                    .charges
                    .front()
                    .is_some_and(|(at, _)| now.duration_since(*at) >= WINDOW)
                {
                    let (_, cost) = budget.charges.pop_front().expect("front checked");
                    budget.used -= cost;
                }
                if budget.used + units <= LIMIT {
                    budget.used += units;
                    budget.charges.push_back((now, units));
                    return;
                }
                let mut released = 0;
                let mut wait = WINDOW;
                for (at, cost) in &budget.charges {
                    released += cost;
                    if budget.used + units - released <= LIMIT {
                        wait = (*at + WINDOW).saturating_duration_since(now);
                        break;
                    }
                }
                wait
            };
            tokio::time::sleep(wait).await;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[tokio::test(start_paused = true)]
    async fn browsing_bursts_then_waits_for_rolling_quota_and_cancellation_is_free() {
        let rate = Arc::new(Rate::new());
        let started = Instant::now();
        for _ in 0..5 {
            rate.acquire(1000).await;
        }
        rate.acquire(500).await;
        assert_eq!(
            Instant::now(),
            started,
            "five metadata pages must not be delayed by local pacing"
        );
        let waiting = rate.clone();
        let task = tokio::spawn(async move {
            waiting.acquire(75).await;
        });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        task.abort();
        tokio::time::advance(Duration::from_secs(59)).await;
        assert_eq!(rate.state.lock().await.used, 5500);
        tokio::time::advance(Duration::from_secs(1)).await;
        let resumed = Instant::now();
        rate.acquire(75).await;
        assert_eq!(Instant::now(), resumed);
        assert_eq!(
            rate.state.lock().await.used,
            75,
            "cancelled waiter did not consume quota"
        );
        Rate::new().acquire(5500).await;
        assert_eq!(Instant::now(), resumed, "accounts have independent budgets");
    }
}
