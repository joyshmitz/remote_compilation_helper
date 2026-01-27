use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct TestEnv {
    vars: HashMap<&'static str, &'static str>,
}

impl TestEnv {
    fn new(pairs: &[(&'static str, &'static str)]) -> Self {
        let vars = pairs.iter().copied().collect();
        Self { vars }
    }

    fn get(&self, key: &str) -> Option<String> {
        self.vars.get(key).map(|value| (*value).to_string())
    }
}

#[test]
fn rate_limiter_allows_first_update() {
    let limiter = RateLimiter::new(10);
    assert!(limiter.allow());
}

#[test]
fn rate_limiter_blocks_rapid_updates() {
    let limiter = RateLimiter::new(10);
    assert!(limiter.allow());
    assert!(!limiter.allow());
}

#[test]
fn rate_limiter_enforces_interval() {
    let limiter = RateLimiter::new(10);
    let interval = limiter.min_interval_ns();

    assert!(limiter.allow_at(0));
    assert!(!limiter.allow_at(interval / 2));
    assert!(limiter.allow_at(interval));
}

#[test]
fn rate_limiter_reset_allows_again() {
    let limiter = RateLimiter::new(10);
    assert!(limiter.allow());
    assert!(!limiter.allow());
    limiter.reset();
    assert!(limiter.allow());
}

#[test]
fn rate_limiter_thread_safe() {
    let limiter = Arc::new(RateLimiter::new(100));
    let count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let limiter = Arc::clone(&limiter);
            let count = Arc::clone(&count);
            std::thread::spawn(move || {
                for _ in 0..200 {
                    if limiter.allow() {
                        count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total = count.load(Ordering::Relaxed);
    assert!(total > 0);
    assert!(total < 200);
}

#[test]
fn terminal_width_detects_columns_env() {
    let mut state = TerminalState::new();
    let env = TestEnv::new(&[("COLUMNS", "120")]);
    state.refresh_width_with(|key| env.get(key));
    assert_eq!(state.width, 120);
}

#[test]
fn terminal_width_falls_back_on_invalid_env() {
    let mut state = TerminalState::new();
    let env = TestEnv::new(&[("COLUMNS", "0")]);
    state.refresh_width_with(|key| env.get(key));
    assert_eq!(state.width, DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn terminal_truncates_to_width() {
    let mut state = TerminalState::new();
    state.width = 5;
    assert_eq!(state.truncate("1234567"), "12345");
}

#[test]
fn terminal_truncates_zero_width_to_single_char() {
    let mut state = TerminalState::new();
    state.width = 0;
    assert_eq!(state.truncate("abcd"), "a");
}

#[test]
fn cleanup_guard_noop_when_disabled() {
    let guard = CleanupGuard::new(false);
    guard.clear_line();
    guard.hide_cursor();
    guard.show_cursor();
}

#[test]
fn progress_context_nested_counts() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    let ctx1 = ProgressContext::new_for_test(true);
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 1);

    let ctx2 = ProgressContext::new_for_test(true);
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 2);

    drop(ctx2);
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 1);

    drop(ctx1);
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 0);
}

#[test]
fn progress_context_disabled_when_not_tty() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    let ctx = ProgressContext::new_for_test(false);
    assert!(!ctx.enabled);
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 0);
}

#[test]
fn progress_context_render_handles_long_lines() {
    let _guard = TEST_LOCK.lock();
    let mut ctx = ProgressContext::new_for_test(false);
    ctx.render("This should be ignored because context is disabled");
}

#[test]
fn signal_state_flags() {
    let state = SignalState::new();
    assert!(!state.interrupted.load(Ordering::SeqCst));
    assert!(!state.take_resized());

    state.simulate_interrupt();
    state.simulate_resize();

    assert!(state.interrupted.load(Ordering::SeqCst));
    assert!(state.take_resized());
    assert!(!state.take_resized());
}

#[test]
fn progress_context_rate_limit_respects_interval() {
    let _guard = TEST_LOCK.lock();
    let mut ctx = ProgressContext::new_for_test(true);
    ctx.rate_limiter = RateLimiter::new(10);

    ctx.render("first");
    ctx.render("second");
    std::thread::sleep(Duration::from_millis(110));
    ctx.render("third");
}

// ==================== Additional Coverage Tests ====================

#[test]
fn detect_terminal_width_with_valid_columns() {
    let width = detect_terminal_width_with(|key| {
        if key == "COLUMNS" {
            Some("100".to_string())
        } else {
            None
        }
    });
    assert_eq!(width, 100);
}

#[test]
fn detect_terminal_width_with_missing_env() {
    let width = detect_terminal_width_with(|_| None);
    assert_eq!(width, DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn detect_terminal_width_with_invalid_value() {
    let width = detect_terminal_width_with(|key| {
        if key == "COLUMNS" {
            Some("not_a_number".to_string())
        } else {
            None
        }
    });
    assert_eq!(width, DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn detect_terminal_width_with_negative_overflow() {
    // Values that would overflow u16 should fall back to default
    let width = detect_terminal_width_with(|key| {
        if key == "COLUMNS" {
            Some("999999".to_string())
        } else {
            None
        }
    });
    // 999999 overflows u16, so parse fails and we get default
    assert_eq!(width, DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn rate_limiter_min_clamps_to_one() {
    // Creating with 0 should clamp to 1 update per second
    let limiter = RateLimiter::new(0);
    assert_eq!(limiter.min_interval_ns(), 1_000_000_000);
}

#[test]
fn rate_limiter_high_rate() {
    let limiter = RateLimiter::new(1000);
    // At 1000/sec, interval should be 1ms (1_000_000 ns)
    assert_eq!(limiter.min_interval_ns(), 1_000_000);
}

#[test]
fn rate_limiter_allow_after_interval_passes() {
    let limiter = RateLimiter::new(10);
    let interval = limiter.min_interval_ns();

    // First call always allowed
    assert!(limiter.allow_at(0));

    // Just before interval - blocked
    assert!(!limiter.allow_at(interval - 1));

    // Exactly at interval - allowed
    assert!(limiter.allow_at(interval));

    // Double interval from last - allowed
    assert!(limiter.allow_at(interval * 2));
}

#[test]
fn terminal_state_new_has_default_width() {
    let state = TerminalState::new();
    // Width should be detected (usually DEFAULT_TERMINAL_WIDTH in test env)
    assert!(state.width > 0);
}

#[test]
fn terminal_truncate_short_string_unchanged() {
    let mut state = TerminalState::new();
    state.width = 100;
    assert_eq!(state.truncate("short"), "short");
}

#[test]
fn terminal_truncate_exact_width() {
    let mut state = TerminalState::new();
    state.width = 5;
    assert_eq!(state.truncate("12345"), "12345");
}

#[test]
fn terminal_truncate_unicode() {
    let mut state = TerminalState::new();
    state.width = 3;
    // Unicode characters should be truncated by char count, not bytes
    assert_eq!(state.truncate("αβγδ"), "αβγ");
}

#[test]
fn cleanup_guard_enabled_operations() {
    // This test verifies the guard can be created with enabled=true
    // We can't easily verify the actual terminal operations in tests
    let guard = CleanupGuard::new(true);
    // Call methods to verify they don't panic
    guard.clear_line();
    guard.hide_cursor();
    guard.show_cursor();
}

#[test]
fn signal_state_new_has_defaults() {
    let state = SignalState::new();
    assert!(!state.interrupted.load(Ordering::SeqCst));
    assert!(!state.resized.load(Ordering::SeqCst));
}

#[test]
fn signal_state_mark_interrupted() {
    let state = SignalState::new();
    state.mark_interrupted();
    assert!(state.interrupted.load(Ordering::SeqCst));
}

#[test]
fn signal_state_mark_resized() {
    let state = SignalState::new();
    state.mark_resized();
    assert!(state.resized.load(Ordering::SeqCst));
}

#[test]
fn signal_state_take_resized_clears_flag() {
    let state = SignalState::new();
    state.mark_resized();
    assert!(state.take_resized()); // First take returns true
    assert!(!state.take_resized()); // Second take returns false (cleared)
}

#[test]
fn progress_context_clear_when_disabled() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    let ctx = ProgressContext::new_for_test(false);
    ctx.clear(); // Should be a no-op when disabled
    assert!(!ctx.enabled);
}

#[test]
fn progress_context_clear_when_enabled() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    let ctx = ProgressContext::new_for_test(true);
    ctx.clear(); // Should attempt to clear line
    assert!(ctx.enabled);
}

#[test]
fn progress_context_render_when_disabled_is_noop() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    let mut ctx = ProgressContext::new_for_test(false);
    // Render should do nothing when disabled
    ctx.render("test line");
    ctx.render("another line");
    // No panic means success
}

#[test]
fn progress_context_drop_decrements_count() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);

    {
        let _ctx = ProgressContext::new_for_test(true);
        assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 1);
    }
    // After drop, count should be back to 0
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 0);
}

#[test]
fn progress_context_drop_disabled_no_decrement() {
    let _guard = TEST_LOCK.lock();
    ACTIVE_CONTEXTS.store(5, Ordering::SeqCst);

    {
        let _ctx = ProgressContext::new_for_test(false);
        // Disabled context shouldn't increment
        assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 5);
    }
    // Disabled context shouldn't decrement on drop
    assert_eq!(ACTIVE_CONTEXTS.load(Ordering::SeqCst), 5);

    // Cleanup
    ACTIVE_CONTEXTS.store(0, Ordering::SeqCst);
}

#[test]
fn rate_limiter_now_ns_returns_elapsed() {
    let limiter = RateLimiter::new(10);
    let first = limiter.now_ns();
    std::thread::sleep(Duration::from_millis(10));
    let second = limiter.now_ns();
    // Second should be greater (at least 10ms = 10_000_000 ns later)
    assert!(second > first);
    assert!(second - first >= 9_000_000); // Allow some timing slack
}

#[test]
fn terminal_width_env_empty_string() {
    let width = detect_terminal_width_with(|key| {
        if key == "COLUMNS" {
            Some("".to_string())
        } else {
            None
        }
    });
    assert_eq!(width, DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn terminal_width_env_whitespace() {
    let width = detect_terminal_width_with(|key| {
        if key == "COLUMNS" {
            Some("  80  ".to_string())
        } else {
            None
        }
    });
    // Whitespace around number should fail to parse
    assert_eq!(width, DEFAULT_TERMINAL_WIDTH);
}
