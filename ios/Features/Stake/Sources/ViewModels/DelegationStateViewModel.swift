// Copyright (c). Gem Wallet. All rights reserved.

import struct BsvHomeCore.BsvHomeDelegationStatus
import Localization
import Style
import SwiftUI

public struct DelegationStateViewModel {
    private let status: BsvHomeDelegationStatus

    public init(status: BsvHomeDelegationStatus) {
        self.status = status
    }

    public var title: String {
        switch status.state {
        case .active: Localized.Stake.active
        case .pending: Localized.Stake.pending
        case .inactive: Localized.Stake.inactive
        case .activating: Localized.Stake.activating
        case .deactivating: Localized.Stake.deactivating
        case .awaitingWithdrawal: Localized.Stake.awaitingWithdrawal
        }
    }

    public var color: Color {
        switch status.tone {
        case .positive: Colors.green
        case .pending: Colors.orange
        case .negative: Colors.red
        }
    }

    public var textStyle: TextStyle {
        TextStyle(font: .callout, color: color)
    }
}
