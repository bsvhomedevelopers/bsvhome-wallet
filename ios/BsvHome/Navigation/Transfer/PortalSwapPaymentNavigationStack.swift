import Components
import Formatters
import Foundation
import typealias BsvHomeCore.BsvHomeBigInt
import enum BsvHomeCore.BsvHomeConfirmError
import enum BsvHomeCore.BsvHomePortalSwapError
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI
import Transfer
import WalletTab

struct PortalSwapPaymentNavigationStack: View {
    @Environment(\.viewModelFactory) private var viewModelFactory
    let context: PortalSwapPaymentContext
    private let transferContext: ConfirmTransferContext
    @State private var isSubmitting = false
    @State private var confirmationModel: ConfirmTransferSceneViewModel?
    @State private var errorMessage: String?

    init(context: PortalSwapPaymentContext, transferContext: ConfirmTransferContext = .standard) {
        self.context = context
        self.transferContext = transferContext
    }

    var body: some View {
        NavigationStack {
            Group {
                if let saved = context.savedPayment {
                    List {
                        Section {
                            ListItemView(model: ListItemModel(title: Localized.Swap.youPay, subtitle: ValueFormatter(style: .full).string(saved.value, asset: context.transfer.asset)), style: .wowPay)
                            ListItemView(model: ListItemModel(title: Localized.wowpaySwapDepositAddress, subtitle: saved.recipientAddress, subtitleLineLimit: nil), style: .wowPay)
                            ListItemView(model: ListItemModel(title: Localized.Transfer.network, subtitle: AssetViewModel(asset: context.transfer.asset).networkFullName), style: .wowPay)
                            ListItemView(model: ListItemModel(title: Localized.Transfer.networkFee, subtitle: savedNetworkFee), style: .wowPay)
                        }
                        Section {
                            ForEach(context.summaryItems.indices, id: \.self) { index in
                                ListItemView(model: context.summaryItems[index], style: .wowPay)
                            }
                        }
                        if let errorMessage {
                            Section {
                                Text(errorMessage).foregroundStyle(Colors.red)
                            }
                        }
                    }
                    .scrollContentBackground(.hidden)
                    .wowPayBackground()
                    .safeAreaButton {
                        StateButton(
                            text: Localized.wowpaySwapPayOrder,
                            type: .wowPayPrimary(isSubmitting ? .loading() : .normal),
                        ) {
                            Task { await rebroadcast() }
                        }
                    }
                    .navigationTitle(Localized.wowpaySwapPayOrder)
                } else if let confirmationModel {
                    ConfirmTransferNavigationView(model: confirmationModel)
                } else {
                    LoadingView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .wowPayBackground()
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button(Localized.Common.cancel, systemImage: SystemImage.xmark) {
                        context.finish(.failure(CancellationError()))
                    }
                    .labelStyle(.iconOnly)
                    .disabled(isSubmitting || confirmationModel?.isConfirming == true)
                }
            }
            .navigationBarTitleDisplayMode(.inline)
        }
        .task {
            guard context.savedPayment == nil, confirmationModel == nil else { return }
            confirmationModel = viewModelFactory.confirmTransferScene(
                wallet: context.wallet,
                data: context.transfer,
                confirmTransferDelegate: { result in
                    Task { @MainActor in
                        guard case let .success(transactionId) = result else { return }
                        await reportBroadcast(transactionId)
                    }
                },
                checkpoint: context.checkpoint,
                additionalDetails: context.summaryItems,
                context: transferContext,
                onComplete: {},
            )
        }
        .interactiveDismissDisabled(true)
        .activityIndicator(isLoading: isSubmitting, message: Localized.Common.loading)
    }

    private var savedNetworkFee: String {
        guard let saved = context.savedPayment,
              let assetId = try? AssetId(id: saved.feeAssetId)
        else { return "—" }
        return ValueFormatter(style: .full).string(BsvHomeBigInt(saved.networkFee), asset: assetId.chain.asset)
    }

    private func rebroadcast() async {
        guard let saved = context.savedPayment, !isSubmitting else { return }
        isSubmitting = true
        errorMessage = nil
        do {
            let transactionId = try await viewModelFactory.broadcastPortalPayment(wallet: context.wallet, transfer: context.transfer, saved: saved, checkpoint: context.checkpoint)
            await reportBroadcast(transactionId)
        } catch BsvHomeConfirmError.Cancelled {
            context.finish(.failure(CancellationError()))
        } catch {
            if error.isCancelled {
                context.finish(.failure(CancellationError()))
            } else {
                errorMessage = error.localizedDescription
            }
        }
        isSubmitting = false
    }

    private func reportBroadcast(_ transactionId: String) async {
        isSubmitting = true
        do {
            guard let saved = try await context.service.savedPayment(orderId: context.id), saved.transactionId == transactionId else {
                throw BsvHomePortalSwapError.BroadcastConflict
            }
        } catch {
            context.finish(.failure(error))
            isSubmitting = false
            return
        }
        do {
            _ = try await context.service.reportSavedBroadcast(orderId: context.id)
            context.finish(.success(()))
        } catch BsvHomePortalSwapError.BroadcastConflict {
            context.finish(.failure(BsvHomePortalSwapError.BroadcastConflict))
        } catch BsvHomePortalSwapError.OrderMismatch {
            context.finish(.failure(BsvHomePortalSwapError.OrderMismatch))
        } catch {
            context.finish(.failure(AnyError(Localized.wowpaySwapBroadcastReportPending)))
        }
        isSubmitting = false
    }
}
