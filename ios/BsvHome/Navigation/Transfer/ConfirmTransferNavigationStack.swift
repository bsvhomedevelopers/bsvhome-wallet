// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import struct BsvHomeCore.BsvHomeTransferData
import Primitives
import Style
import SwiftUI
import Transfer

struct ConfirmTransferNavigationStack: View {
    @Environment(\.viewModelFactory) private var viewModelFactory
    @State private var model: ConfirmTransferSceneViewModel?

    private let wallet: Wallet
    private let transferData: BsvHomeTransferData
    private let onComplete: VoidAction

    init(
        wallet: Wallet,
        transferData: BsvHomeTransferData,
        onComplete: VoidAction,
    ) {
        self.wallet = wallet
        self.transferData = transferData
        self.onComplete = onComplete
    }

    var body: some View {
        NavigationStack {
            Group {
                if let model {
                    ConfirmTransferNavigationView(model: model)
                } else {
                    LoadingView()
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("", systemImage: SystemImage.xmark) {
                        model?.cancelExecution()
                        onComplete?()
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .interactiveDismissDisabled(true)
            .onAppear {
                if model == nil {
                    model = viewModelFactory.confirmTransferScene(wallet: wallet, data: transferData, onComplete: onComplete)
                }
            }
        }
    }
}
