// Copyright (c). Gem Wallet. All rights reserved.

import Localization
import Primitives
import Style
import SwiftUI
import enum BsvHomeCore.BsvHomeSimulationWarningKind
import struct BsvHomeCore.BsvHomeSimulationWarningRow
import enum BsvHomeCore.SimulationSeverity

public struct SimulationWarningViewModel: Identifiable {
    private let row: BsvHomeSimulationWarningRow

    public init(row: BsvHomeSimulationWarningRow) {
        self.row = row
    }

    public var id: BsvHomeSimulationWarningRow {
        row
    }

    public var title: String? {
        if row.kind == .validationError, row.severity != .critical {
            return Localized.Common.warning
        }
        return warningDetails == nil ? nil : row.kind.warningTitle
    }

    public var message: String {
        if row.kind == .validationError, row.severity != .critical {
            return row.message ?? ""
        }
        return warningDetails ?? row.kind.warningTitle
    }

    public var color: Color {
        row.severity.color
    }

    private var warningDetails: String? {
        row.message ?? row.kind.defaultMessage
    }
}

private extension BsvHomeSimulationWarningKind {
    var warningTitle: String {
        switch self {
        case .unlimitedApproval: Localized.Simulation.Warning.UnlimitedTokenApproval.title
        case .nftCollectionApproval: Localized.Simulation.Warning.NftCollectionApproval.title
        case .externallyOwnedSpender: Localized.Common.warning
        case .suspiciousSpender, .validationError: Localized.Errors.errorOccurred
        }
    }

    var defaultMessage: String? {
        switch self {
        case .unlimitedApproval: Localized.Simulation.Warning.UnlimitedTokenApproval.description
        case .validationError: Localized.Errors.errorOccurred
        case .externallyOwnedSpender: Localized.Simulation.warningExternallyOwnedSpenderDescription
        case .suspiciousSpender: Localized.Common.suspiciousAddress
        case .nftCollectionApproval: nil
        }
    }
}

private extension SimulationSeverity {
    var color: Color {
        switch self {
        case .critical:
            Colors.red
        case .low, .warning:
            Colors.orange
        }
    }
}
