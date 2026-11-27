// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeLockPeriod
import Primitives

public extension LockPeriod {
    var bsvHomeLockPeriod: BsvHomeLockPeriod {
        switch self {
        case .immediate: .immediate
        case .oneMinute: .oneMinute
        case .fiveMinutes: .fiveMinutes
        case .fifteenMinutes: .fifteenMinutes
        case .oneHour: .oneHour
        case .sixHours: .sixHours
        }
    }
}
