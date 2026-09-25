//! Real gain of a cleanup (SPEC section 8).
//!
//! macOS gives space back with a delay: after deleting 25 simulators, free space first grew
//! by 0.6 GB, then by about 31 GB a few minutes later. So free space is sampled after the
//! action until it stops moving, and only then is the gain reported as final.

use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct StabilityConfig {
    /// Time between two samples.
    pub interval: Duration,
    /// Never conclude before this, even if free space looks stable.
    pub min_observation: Duration,
    /// Same, while the gain is below half the estimate: the release may simply not have happened yet.
    pub patient_observation: Duration,
    /// Free space must stay within `tolerance` for this long.
    pub window: Duration,
    /// Shorter window used once the gain reaches `early_ratio` of the estimate.
    pub early_window: Duration,
    pub early_ratio: f64,
    /// Variation considered as noise (other apps write to the disk too).
    pub tolerance: u64,
    /// Give up waiting and report the last gain.
    pub timeout: Duration,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        StabilityConfig {
            interval: Duration::from_secs(5),
            min_observation: Duration::from_secs(90),
            patient_observation: Duration::from_secs(4 * 60),
            window: Duration::from_secs(60),
            early_window: Duration::from_secs(15),
            early_ratio: 0.9,
            tolerance: 64 * 1024 * 1024,
            timeout: Duration::from_secs(6 * 60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GainState {
    /// Free space is still moving: "récupération en cours".
    Recovering,
    Stable,
    /// Stopped waiting before free space settled.
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GainProgress {
    /// Free space now minus free space before the action. Can be negative if other apps wrote meanwhile.
    pub gain: i64,
    pub estimate: u64,
    pub elapsed_secs: u64,
    pub state: GainState,
}

/// Decides when free space has settled. Pure logic, fed with samples.
#[derive(Debug)]
struct Stabilizer {
    config: StabilityConfig,
    baseline: u64,
    estimate: u64,
    samples: Vec<(Duration, u64)>,
}

impl Stabilizer {
    fn new(baseline: u64, estimate: u64, config: StabilityConfig) -> Self {
        Stabilizer { config, baseline, estimate, samples: Vec::new() }
    }

    fn push(&mut self, elapsed: Duration, free: u64) -> GainProgress {
        self.samples.push((elapsed, free));
        let gain = free as i64 - self.baseline as i64;
        let tolerance = self.config.tolerance;
        let stable_since = self
            .samples
            .iter()
            .rev()
            .take_while(|(_, f)| f.abs_diff(free) <= tolerance)
            .last()
            .map(|(t, _)| *t)
            .unwrap_or(elapsed);
        let steady_for = elapsed.saturating_sub(stable_since);
        let reached = |ratio: f64| gain > 0 && gain as f64 >= self.estimate as f64 * ratio;
        let near_estimate = self.estimate > 0 && reached(self.config.early_ratio);
        let min_observation = if self.estimate > 0 && !reached(0.5) {
            self.config.patient_observation
        } else {
            self.config.min_observation
        };

        let settled_early = near_estimate && steady_for >= self.config.early_window;
        let settled = elapsed >= min_observation && steady_for >= self.config.window;
        let state = if settled_early || settled {
            GainState::Stable
        } else if elapsed >= self.config.timeout {
            GainState::TimedOut
        } else {
            GainState::Recovering
        };
        GainProgress { gain, estimate: self.estimate, elapsed_secs: elapsed.as_secs(), state }
    }
}

/// Samples free space until it settles, reporting each step.
pub fn follow(
    baseline: u64,
    estimate: u64,
    config: StabilityConfig,
    mut sample: impl FnMut() -> std::io::Result<u64>,
    mut sleep: impl FnMut(Duration),
    mut on_progress: impl FnMut(GainProgress),
) -> GainProgress {
    let mut stabilizer = Stabilizer::new(baseline, estimate, config);
    let mut elapsed = Duration::ZERO;
    let mut last = GainProgress { gain: 0, estimate, elapsed_secs: 0, state: GainState::Recovering };
    loop {
        if let Ok(free) = sample() {
            last = stabilizer.push(elapsed, free);
            on_progress(last);
            if last.state != GainState::Recovering {
                return last;
            }
        } else if elapsed >= config.timeout {
            let last = GainProgress { state: GainState::TimedOut, ..last };
            on_progress(last);
            return last;
        }
        sleep(config.interval);
        elapsed += config.interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    fn run(estimate: u64, curve: impl Fn(u64) -> u64) -> (GainProgress, usize) {
        let now = std::cell::Cell::new(0u64);
        let mut steps = 0;
        let result = follow(
            100 * GB,
            estimate,
            StabilityConfig::default(),
            || Ok(100 * GB + curve(now.get())),
            |d| now.set(now.get() + d.as_secs()),
            |_| steps += 1,
        );
        (result, steps)
    }

    #[test]
    fn waits_for_delayed_release() {
        // The simulators case: 0.6 GB at first, 31 GB after three minutes.
        let (result, _) = run(31 * GB, |t| if t < 180 { GB * 6 / 10 } else { 31 * GB });
        assert_eq!(result.state, GainState::Stable);
        assert_eq!(result.gain, 31 * GB as i64);
        assert!(result.elapsed_secs >= 180);
    }

    #[test]
    fn finishes_early_when_estimate_is_reached() {
        let (result, _) = run(10 * GB, |_| 10 * GB);
        assert_eq!(result.state, GainState::Stable);
        assert_eq!(result.elapsed_secs, 15);
    }

    #[test]
    fn concludes_on_a_stable_plateau() {
        // du overestimated a little: the real gain never moves.
        let (result, _) = run(10 * GB, |_| 7 * GB);
        assert_eq!(result.state, GainState::Stable);
        assert_eq!(result.gain, 7 * GB as i64);
        assert_eq!(result.elapsed_secs, 90);
    }

    #[test]
    fn waits_longer_on_a_plateau_far_below_estimate() {
        // Hard links or APFS clones: the real gain is much smaller than du said.
        let (result, _) = run(10 * GB, |_| 2 * GB);
        assert_eq!(result.state, GainState::Stable);
        assert_eq!(result.elapsed_secs, 240);
    }

    #[test]
    fn times_out_when_free_space_keeps_moving() {
        let (result, _) = run(0, |t| t * 100 * 1024 * 1024);
        assert_eq!(result.state, GainState::TimedOut);
        assert_eq!(result.elapsed_secs, 360);
    }
}
