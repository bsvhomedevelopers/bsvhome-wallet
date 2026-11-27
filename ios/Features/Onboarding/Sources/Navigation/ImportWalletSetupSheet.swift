import Primitives
import PrimitivesComponents
import SwiftUI

public struct ImportWalletSetupSheet: View {
    @State private var model: ImportWalletViewModel

    private let wallet: Wallet
    private let onComplete: () -> Void

    public init(model: ImportWalletViewModel, wallet: Wallet, onComplete: @escaping () -> Void) {
        _model = State(initialValue: model)
        self.wallet = wallet
        self.onComplete = onComplete
    }

    public var body: some View {
        NavigationStack {
            SetupWalletScene(model: model.setupWalletModel(wallet: wallet) { _ in onComplete() })
                .navigationBarBackButtonHidden()
                .interactiveDismissDisabled()
        }
        .sheet(item: $model.isPresentingSelectImageWallet) { wallet in
            NavigationStack {
                WalletImageScene(model: model.walletImageModel(wallet: wallet))
                    .toolbarDismissItem(type: .close, placement: .topBarLeading)
            }
        }
    }
}
