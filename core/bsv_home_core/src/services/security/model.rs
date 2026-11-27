#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeLockPeriod {
    Immediate,
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    SixHours,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAuthPromptOutcome {
    CancelledByUser,
    CancelledBySystem,
    Unavailable,
    LockedOut,
    Transient,
    Failed,
}
