import Components
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style
import SwiftUI
import Transfer
import WalletTab

struct TrxAcquisitionNavigationStack: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var model: WowPayPortalSwapViewModel
    @State private var paymentPresenter: PortalSwapPaymentPresenter
    private let wallet: Wallet
    private let walletModel: WalletSceneViewModel
    private let viewModelFactory: ViewModelFactory

    init(wallet: Wallet, viewModelFactory: ViewModelFactory) {
        self.wallet = wallet
        self.viewModelFactory = viewModelFactory
        let presenter = PortalSwapPaymentPresenter()
        let source = PortalSwapPaymentSource.acquisition(UUID())
        _paymentPresenter = State(initialValue: presenter)
        _model = State(initialValue: viewModelFactory.portalSwapScene(entryMode: .acquireTrx) { orderId, service in
            try await presenter.present(
                wallet: wallet,
                source: source,
                orderId: orderId,
                service: service,
                isActive: { viewModelFactory.walletSessionService.currentWalletId == wallet.id },
            )
        })
        walletModel = viewModelFactory.walletScene(
            wallet: wallet,
            isPresentingSelectedAssetInput: .constant(nil),
            isPresentingWallets: .constant(false),
        )
    }

    var body: some View {
        NavigationStack {
            WowPaySwapAcquisitionScene(model: model, walletModel: walletModel)
                .navigationTitle(Localized.Wallet.swap)
                .navigationBarTitleDisplayMode(.inline)
                .toolbarDismissItem(type: .close, placement: .topBarLeading)
        }
        .presentationDetents([.large])
        .presentationBackground(Colors.WowPay.background)
        .sheet(item: paymentBinding) { payment in
            PortalSwapPaymentNavigationStack(context: payment, transferContext: .trxAcquisition)
        }
        .onAppear { paymentPresenter.activate() }
        .onDisappear {
            paymentPresenter.deactivate()
            model.onDisappear()
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                paymentPresenter.cancel()
            }
        }
        .onChange(of: viewModelFactory.walletSessionService.currentWalletId) { _, walletId in
            if walletId != wallet.id {
                paymentPresenter.deactivate()
                model.onDisappear()
                dismiss()
            }
        }
    }

    private var paymentBinding: Binding<PortalSwapPaymentContext?> {
        Binding(
            get: { paymentPresenter.payment },
            set: { payment in
                if payment == nil {
                    paymentPresenter.cancel()
                }
            },
        )
    }
}
