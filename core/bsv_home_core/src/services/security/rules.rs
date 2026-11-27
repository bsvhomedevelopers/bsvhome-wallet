use super::model::{BsvHomeAuthPromptOutcome, BsvHomeLockPeriod};

const MILLISECONDS_PER_MINUTE: u32 = 60 * 1_000;

#[uniffi::export]
impl BsvHomeLockPeriod {
    pub fn minutes(self) -> u32 {
        match self {
            Self::Immediate => 0,
            Self::OneMinute => 1,
            Self::FiveMinutes => 5,
            Self::FifteenMinutes => 15,
            Self::OneHour => 60,
            Self::SixHours => 6 * 60,
        }
    }
}

impl BsvHomeLockPeriod {
    fn milliseconds(self) -> u32 {
        self.minutes() * MILLISECONDS_PER_MINUTE
    }
}

#[uniffi::export]
impl BsvHomeAuthPromptOutcome {
    pub fn is_cancelled(self) -> bool {
        match self {
            Self::CancelledByUser | Self::CancelledBySystem => true,
            Self::Unavailable | Self::LockedOut | Self::Transient | Self::Failed => false,
        }
    }

    pub fn retry_delay_milliseconds(self) -> Option<u32> {
        match self {
            Self::CancelledByUser | Self::CancelledBySystem => Some(500),
            Self::Transient => Some(1_000),
            Self::LockedOut => Some(30_000),
            Self::Unavailable | Self::Failed => None,
        }
    }
}

#[uniffi::export]
pub fn lock_periods() -> Vec<BsvHomeLockPeriod> {
    vec![
        BsvHomeLockPeriod::Immediate,
        BsvHomeLockPeriod::OneMinute,
        BsvHomeLockPeriod::FiveMinutes,
        BsvHomeLockPeriod::FifteenMinutes,
        BsvHomeLockPeriod::OneHour,
        BsvHomeLockPeriod::SixHours,
    ]
}

#[uniffi::export]
pub fn lock_period_from_minutes(minutes: Option<u32>) -> BsvHomeLockPeriod {
    minutes
        .and_then(|minutes| lock_periods().into_iter().find(|period| period.minutes() == minutes))
        .unwrap_or(BsvHomeLockPeriod::OneMinute)
}

pub(super) fn should_relock(elapsed_milliseconds: i64, lock_interval_minutes: u32, auth_required: bool, has_pending_request: bool) -> bool {
    let period = lock_period_from_minutes(Some(lock_interval_minutes));
    auth_required && !has_pending_request && elapsed_milliseconds > i64::from(period.milliseconds())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_a_recoverable_prompt_outcome_is_retried() {
        assert_eq!(BsvHomeAuthPromptOutcome::CancelledByUser.retry_delay_milliseconds(), Some(500));
        assert_eq!(BsvHomeAuthPromptOutcome::Transient.retry_delay_milliseconds(), Some(1_000));
        assert_eq!(BsvHomeAuthPromptOutcome::LockedOut.retry_delay_milliseconds(), Some(30_000));
        assert_eq!(
            BsvHomeAuthPromptOutcome::Unavailable.retry_delay_milliseconds(),
            None,
            "no enrolled biometry cannot be retried into working"
        );
        assert_eq!(BsvHomeAuthPromptOutcome::Failed.retry_delay_milliseconds(), None);
    }

    #[test]
    fn test_cancellation_covers_both_the_user_and_the_system() {
        assert!(BsvHomeAuthPromptOutcome::CancelledByUser.is_cancelled());
        assert!(BsvHomeAuthPromptOutcome::CancelledBySystem.is_cancelled());
        assert!(!BsvHomeAuthPromptOutcome::LockedOut.is_cancelled());
        assert!(!BsvHomeAuthPromptOutcome::Failed.is_cancelled());
    }

    #[test]
    fn test_lock_periods_carry_the_same_minutes_on_both_platforms() {
        let minutes: Vec<u32> = lock_periods().into_iter().map(BsvHomeLockPeriod::minutes).collect();
        assert_eq!(minutes, vec![0, 1, 5, 15, 60, 360]);
        assert_eq!(BsvHomeLockPeriod::SixHours.milliseconds(), 21_600_000);
        assert_eq!(lock_period_from_minutes(Some(15)), BsvHomeLockPeriod::FifteenMinutes);
        assert_eq!(
            lock_period_from_minutes(Some(7)),
            BsvHomeLockPeriod::OneMinute,
            "an unknown stored value falls back to the default"
        );
        assert_eq!(lock_period_from_minutes(None), BsvHomeLockPeriod::OneMinute, "a missing stored value is the default");
    }

    #[test]
    fn test_relock_needs_auth_no_pending_request_and_an_elapsed_period() {
        assert!(should_relock(60_001, 1, true, false));
        assert!(!should_relock(60_000, 1, true, false), "the period has to be exceeded, not merely reached");
        assert!(!should_relock(60_001, 1, false, false), "no lock when authentication is off");
        assert!(!should_relock(60_001, 1, true, true), "a request in flight holds the lock off");
        assert!(should_relock(1, 0, true, false), "immediate locks as soon as any time has passed");
        assert!(!should_relock(0, 0, true, false));
        assert!(should_relock(60_001, 7, true, false), "an unknown stored interval falls back to the default period");
    }
}
