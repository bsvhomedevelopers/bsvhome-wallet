// Copyright (c). Gem Wallet. All rights reserved.

import Perpetuals
import Primitives
import SwiftUI
import struct BsvHomeCore.BsvHomeTransferData

struct AutocloseNavigationStack: View {
    @Environment(\.viewModelFactory) private var viewModelFactory

    @State private var navigationPath = NavigationPath()

    let position: PerpetualPositionData
    let wallet: Wallet
    let onComplete: VoidAction

    var body: some View {
        NavigationStack(path: $navigationPath) {
            AutocloseScene(
                model: AutocloseSceneViewModel(
                    type: .modify(position, onTransferAction: { navigationPath.append($0) }),
                ),
            )
            .navigationDestination(for: BsvHomeTransferData.self) {
                ConfirmTransferNavigationView(
                    model: viewModelFactory.confirmTransferScene(
                        wallet: wallet,
                        data: $0,
                        onComplete: onComplete,
                    ),
                )
            }
        }
    }
}
