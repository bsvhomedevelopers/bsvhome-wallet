// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import SwiftUI
import struct BsvHomeCore.BsvHomeSimulationWarningRow

public struct SimulationWarningsContent: View {
    private let warnings: [BsvHomeSimulationWarningRow]

    public init(warnings: [BsvHomeSimulationWarningRow]) {
        self.warnings = warnings
    }

    public var body: some View {
        ForEach(warnings.map(SimulationWarningViewModel.init)) {
            ListItemErrorView(
                errorTitle: $0.title,
                errorImageColor: $0.color,
                error: AnyError($0.message),
            )
        }
    }
}
