//! In-memory benchmark queue with rate limiting.

#![allow(dead_code)] // Used by SpeedScore API endpoints and future scheduler work.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rch_common::WorkerId;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Benchmark request queued by the API.
#[derive(Debug, Clone)]
pub struct BenchmarkRequest {
    pub request_id: String,
    pub worker_id: WorkerId,
    pub requested_at: DateTime<Utc>,
}

/// Rate limit information when benchmark trigger is rejected.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub retry_after: ChronoDuration,
    pub last_triggered_at: DateTime<Utc>,
}

/// Simple FIFO queue for benchmark triggers.
pub struct BenchmarkQueue {
    min_interval: ChronoDuration,
    queue: Mutex<VecDeque<BenchmarkRequest>>,
    last_triggered: Mutex<HashMap<WorkerId, DateTime<Utc>>>,
}

impl BenchmarkQueue {
    /// Create a new queue with the specified per-worker minimum interval.
    pub fn new(min_interval: ChronoDuration) -> Self {
        Self {
            min_interval,
            queue: Mutex::new(VecDeque::new()),
            last_triggered: Mutex::new(HashMap::new()),
        }
    }

    /// Attempt to enqueue a benchmark request.
    pub fn enqueue(
        &self,
        worker_id: WorkerId,
        request_id: String,
    ) -> Result<BenchmarkRequest, RateLimitInfo> {
        let now = Utc::now();
        {
            let last = self.last_triggered.lock().expect("benchmark rate lock");
            if let Some(last_at) = last.get(&worker_id) {
                let since = now - *last_at;
                if since < self.min_interval {
                    return Err(RateLimitInfo {
                        retry_after: self.min_interval - since,
                        last_triggered_at: *last_at,
                    });
                }
            }
        }

        {
            let mut last = self.last_triggered.lock().expect("benchmark rate lock");
            last.insert(worker_id.clone(), now);
        }

        let request = BenchmarkRequest {
            request_id,
            worker_id,
            requested_at: now,
        };

        let mut queue = self.queue.lock().expect("benchmark queue lock");
        queue.push_back(request.clone());
        Ok(request)
    }

    /// Current queued depth.
    pub fn len(&self) -> usize {
        let queue = self.queue.lock().expect("benchmark queue lock");
        queue.len()
    }
}
