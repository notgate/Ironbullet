use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub struct DataPool {
    lines: Vec<String>,
    index: AtomicUsize,
    retry_queue: Mutex<Vec<(String, u32)>>,
    /// Credentials that exhausted max_retries due to transient errors.
    /// After the main pool drains, these are replayed for one final pass.
    error_queue: Mutex<Vec<String>>,
    /// Whether the error replay pass has already been triggered.
    error_replayed: AtomicBool,
    /// Lines handed to workers that have not yet reached a terminal/retry decision.
    /// Error replay must wait for this to reach zero because another worker may
    /// still enqueue retryable work after the main list has been claimed.
    in_flight: AtomicUsize,
}

impl DataPool {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            index: AtomicUsize::new(0),
            retry_queue: Mutex::new(Vec::new()),
            error_queue: Mutex::new(Vec::new()),
            error_replayed: AtomicBool::new(false),
            in_flight: AtomicUsize::new(0),
        }
    }

    pub fn with_limits(lines: Vec<String>, skip: usize, take: usize) -> Self {
        Self::new(apply_limits(lines, skip, take))
    }

    pub fn from_file(path: &str, skip_empty: bool) -> std::io::Result<Self> {
        Self::from_file_with_limits(path, skip_empty, 0, 0)
    }

    pub fn from_file_with_limits(
        path: &str,
        skip_empty: bool,
        skip: usize,
        take: usize,
    ) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let lines: Vec<String> = content
            .lines()
            .filter(|l| !skip_empty || !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect();
        Ok(Self::with_limits(lines, skip, take))
    }

    pub fn next_line(&self) -> Option<(String, u32)> {
        // 1) Prioritise retry queue (credentials that failed transiently)
        if let Ok(mut queue) = self.retry_queue.lock() {
            if let Some(entry) = queue.pop() {
                self.in_flight.fetch_add(1, Ordering::AcqRel);
                return Some(entry);
            }
        }
        // 2) Main sequential pool
        let idx = self.index.fetch_add(1, Ordering::Relaxed);
        if let Some(line) = self.lines.get(idx) {
            self.in_flight.fetch_add(1, Ordering::AcqRel);
            return Some((line.clone(), 0));
        }
        // 3) Replay retry-exhausted errors once only after every previously
        // claimed line has settled. A peer may still enqueue an error after
        // another worker sees the main list exhausted.
        if self.in_flight.load(Ordering::Acquire) == 0
            && !self.error_replayed.swap(true, Ordering::SeqCst)
        {
            if let Ok(mut errors) = self.error_queue.lock() {
                if !errors.is_empty() {
                    let mut retries = self.retry_queue.lock().unwrap_or_else(|e| e.into_inner());
                    let count = errors.len();
                    for line in errors.drain(..) {
                        retries.push((line, 0));
                    }
                    eprintln!("[data_pool] replaying {count} errored credentials for final pass");
                    if let Some(entry) = retries.pop() {
                        self.in_flight.fetch_add(1, Ordering::AcqRel);
                        return Some(entry);
                    }
                }
            }
        }
        None
    }

    /// Mark a line returned by `next_line` as settled after its result has been
    /// recorded or it has been placed back into a retry/error queue.
    pub fn finish_attempt(&self) {
        let previous = self.in_flight.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "DataPool::finish_attempt without a claim");
    }

    pub fn has_in_flight(&self) -> bool {
        self.in_flight.load(Ordering::Acquire) > 0
    }

    pub fn return_line(&self, line: String, retry_count: u32) {
        if let Ok(mut queue) = self.retry_queue.lock() {
            queue.push((line, retry_count));
        }
    }

    /// Stash a credential that exhausted max_retries due to transient errors.
    /// These will be replayed once the main pool drains.
    pub fn stash_error(&self, line: String) {
        if let Ok(mut queue) = self.error_queue.lock() {
            queue.push(line);
        }
    }

    pub fn total(&self) -> usize {
        self.lines.len()
    }

    pub fn consumed(&self) -> usize {
        self.index.load(Ordering::Relaxed).min(self.lines.len())
    }

    pub fn remaining(&self) -> usize {
        let idx = self.index.load(Ordering::Relaxed);
        let retry_count = self.retry_queue.lock().map(|q| q.len()).unwrap_or(0);
        self.lines.len().saturating_sub(idx) + retry_count
    }
}

fn apply_limits(lines: Vec<String>, skip: usize, take: usize) -> Vec<String> {
    let iter = lines.into_iter().skip(skip);

    if take == 0 {
        iter.collect()
    } else {
        iter.take(take).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::DataPool;

    #[test]
    fn with_limits_skips_prefix_before_processing() {
        let pool = DataPool::with_limits(
            vec![
                "line1".into(),
                "line2".into(),
                "line3".into(),
                "line4".into(),
            ],
            2,
            0,
        );

        assert_eq!(pool.total(), 2);
        assert_eq!(pool.next_line(), Some(("line3".into(), 0)));
        assert_eq!(pool.next_line(), Some(("line4".into(), 0)));
        assert_eq!(pool.next_line(), None);
    }

    #[test]
    fn with_limits_applies_take_after_skip() {
        let pool = DataPool::with_limits(
            vec![
                "line1".into(),
                "line2".into(),
                "line3".into(),
                "line4".into(),
            ],
            1,
            2,
        );

        assert_eq!(pool.total(), 2);
        assert_eq!(pool.next_line(), Some(("line2".into(), 0)));
        assert_eq!(pool.next_line(), Some(("line3".into(), 0)));
        assert_eq!(pool.next_line(), None);
    }

    #[test]
    fn replays_errors_added_after_the_main_pool_is_claimed() {
        let pool = DataPool::new(vec!["line1".into()]);
        assert_eq!(pool.next_line(), Some(("line1".into(), 0)));
        // A second worker reaches the end while the first is still processing.
        assert_eq!(pool.next_line(), None);
        assert!(pool.has_in_flight());

        pool.stash_error("line1".into());
        pool.finish_attempt();
        assert_eq!(pool.next_line(), Some(("line1".into(), 0)));
        pool.finish_attempt();
        assert_eq!(pool.next_line(), None);
    }
}
