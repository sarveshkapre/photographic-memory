use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CaptureSchedule {
    pub every: Duration,
    pub run_for: Duration,
}

impl CaptureSchedule {
    pub fn validate(&self) -> Result<(), String> {
        if self.every.is_zero() {
            return Err("interval must be greater than 0".to_string());
        }
        if self.run_for.is_zero() {
            return Err("duration must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    every: Duration,
    run_for: Duration,
    next_due: Duration,
}

impl Scheduler {
    pub fn new(schedule: CaptureSchedule) -> Result<Self, String> {
        schedule.validate()?;
        Ok(Self {
            every: schedule.every,
            run_for: schedule.run_for,
            next_due: Duration::ZERO,
        })
    }

    pub fn is_finished(&self, elapsed: Duration) -> bool {
        elapsed >= self.run_for
    }

    pub fn should_capture(&self, elapsed: Duration) -> bool {
        elapsed >= self.next_due && !self.is_finished(elapsed)
    }

    pub fn time_until_next_capture(&self, elapsed: Duration) -> Option<Duration> {
        if self.is_finished(elapsed) {
            return None;
        }
        Some(self.next_due.saturating_sub(elapsed))
    }

    pub fn mark_captured(&mut self) {
        self.next_due = self.next_due.saturating_add(self.every);
    }

    /// Align the next due time to "now" (elapsed since session start).
    ///
    /// This is used when resuming after a pause so the engine does not "catch up"
    /// by issuing a burst of back-to-back captures for missed intervals.
    pub fn align_next_due(&mut self, elapsed: Duration) {
        if !self.is_finished(elapsed) {
            self.next_due = elapsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureSchedule, Scheduler};
    use std::time::Duration;

    #[test]
    fn rejects_invalid_schedule() {
        let zero_interval = CaptureSchedule {
            every: Duration::ZERO,
            run_for: Duration::from_secs(1),
        };
        assert!(Scheduler::new(zero_interval).is_err());

        let zero_run = CaptureSchedule {
            every: Duration::from_secs(1),
            run_for: Duration::ZERO,
        };
        assert!(Scheduler::new(zero_run).is_err());
    }

    #[test]
    fn captures_immediately_then_on_interval() {
        let mut scheduler = Scheduler::new(CaptureSchedule {
            every: Duration::from_secs(2),
            run_for: Duration::from_secs(10),
        })
        .expect("valid scheduler");

        assert!(scheduler.should_capture(Duration::ZERO));
        scheduler.mark_captured();

        assert!(!scheduler.should_capture(Duration::from_millis(1500)));
        assert!(scheduler.should_capture(Duration::from_secs(2)));
    }

    #[test]
    fn stops_after_duration() {
        let scheduler = Scheduler::new(CaptureSchedule {
            every: Duration::from_secs(1),
            run_for: Duration::from_secs(5),
        })
        .expect("valid scheduler");

        assert!(!scheduler.is_finished(Duration::from_secs(4)));
        assert!(scheduler.is_finished(Duration::from_secs(5)));
        assert!(
            scheduler
                .time_until_next_capture(Duration::from_secs(5))
                .is_none()
        );
    }
}
