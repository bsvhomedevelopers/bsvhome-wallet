import Components
import Formatters
import Foundation
import struct BsvHomeCore.BsvHomePortalSwapOrder
import Localization
import PrimitivesComponents
import Style
import SwiftUI

struct WowPayPortalOrderScene: View {
    private let model: WowPayPortalSwapViewModel
    @State private var toast: ToastMessage?

    init(model: WowPayPortalSwapViewModel) {
        self.model = model
    }

    var body: some View {
        @Bindable var model = model
        ScrollView {
            if let order = model.selectedOrder {
                VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                    summary(order)
                    state(order)
                    addresses(order)
                    transactions(order)
                    actions(order)
                    if let error = model.orderError {
                        ListItemErrorView(errorTitle: Localized.Errors.errorOccurred, error: NSError(domain: "PortalSwapOrder", code: 0, userInfo: [NSLocalizedDescriptionKey: error]))
                    }
                    Button(Localized.Common.refresh) {
                        Task { await model.refreshOrder(order.orderId) }
                    }
                    .buttonStyle(.wowPaySecondary())
                    .disabled(model.isBusy)
                }
                .foregroundStyle(Colors.WowPay.heading)
                .padding(Spacing.wowPay.page)
            }
        }
        .wowPayBackground()
        .navigationTitle(Localized.Common.details)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .task(id: model.selectedOrder?.orderId) {
            guard let orderId = model.selectedOrder?.orderId else { return }
            while !Task.isCancelled {
                await model.refreshOrder(orderId)
                if model.selectedOrder?.resolutionStatus == .settled {
                    return
                }
                do {
                    try await Task.sleep(for: .seconds(10))
                } catch {
                    return
                }
            }
        }
        .safariSheet(url: $model.actionUrl)
        .onChange(of: model.actionUrl) { previous, current in
            if previous != nil, current == nil, let order = model.selectedOrder {
                Task { await model.refreshOrder(order.orderId) }
            }
        }
        .toast(message: $toast)
    }

    private func summary(_ order: BsvHomePortalSwapOrder) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            row(Localized.wowpaySwapOrderId, value: order.orderId, copiesValue: true)
            row(Localized.Swap.youPay, value: order.fromAmountDisplay ?? "—")
            row(Localized.wowpaySwapEstimatedReceive, value: order.estimatedToAmountDisplay ?? "—")
            if let received = order.actualToAmountDisplay {
                row(Localized.wowpaySwapActualReceived, value: received)
            }
            row(Localized.Transfer.network, value: order.to?.network ?? "—")
            row(Localized.Common.provider, value: order.providerDisplayName)
            Text(Localized.wowpaySwapFloatingRateDescription)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
        }
        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner)
    }

    private func state(_ order: BsvHomePortalSwapOrder) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            row(Localized.Transaction.status, value: WowPayPortalOrderLabels.phase(order.orderStatus))
            if order.broadcastReportPending {
                Text(Localized.wowpaySwapBroadcastReportPending)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            if order.orderStatus == .completed {
                Text(order.payoutVerified ? Localized.wowpaySwapPayoutVerified : Localized.wowpaySwapPayoutVerificationPending)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            if let verification = WowPayPortalOrderLabels.verification(order.verificationStatus) {
                row(Localized.wowpaySwapVerification, value: verification)
            }
            if let synchronization = WowPayPortalOrderLabels.synchronization(order.syncStatus) {
                row(Localized.wowpaySwapSyncStatus, value: synchronization)
            }
            row(Localized.wowpaySwapResolutionStatus, value: WowPayPortalOrderLabels.settlement(order.resolutionStatus))
            if order.orderStatus == .onHold, let phase = order.lastActivePhase {
                row(Localized.Transaction.Status.pending, value: WowPayPortalOrderLabels.phase(phase))
            }
            row(Localized.Transaction.date, value: RelativeDateFormatter().string(from: Date(timeIntervalSince1970: TimeInterval(order.updatedAt))))
            if let expiry = order.depositExpiresAt {
                row(Localized.Common.expiration, value: RelativeDateFormatter().string(from: Date(timeIntervalSince1970: TimeInterval(expiry))))
            }
            if let reason = order.reasonCode {
                row(Localized.Common.description, value: reason)
            }
        }
        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner, surface: Colors.WowPay.pale)
    }

    private func addresses(_ order: BsvHomePortalSwapOrder) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            row(Localized.wowpaySwapDepositAddress, value: order.depositAddress ?? "—", copiesValue: order.depositAddress != nil)
            if let extraId = order.depositExtraId {
                row(Localized.Transfer.memo, value: extraId, copiesValue: true)
            }
            row(Localized.Transaction.recipient, value: order.payoutAddress, copiesValue: true)
            row(Localized.wowpaySwapRefundAddress, value: order.refundAddress, copiesValue: true)
        }
        .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner)
    }

    @ViewBuilder
    private func transactions(_ order: BsvHomePortalSwapOrder) -> some View {
        if order.sourceTxid != nil || order.payoutTxid != nil || order.refundTxid != nil {
            VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
                if let transactionId = order.sourceTxid {
                    row(Localized.wowpaySwapSourceTransaction, value: transactionId, copiesValue: true)
                }
                if let transactionId = order.payoutTxid {
                    row(Localized.wowpaySwapPayoutTransaction, value: transactionId, copiesValue: true)
                }
                if let transactionId = order.refundTxid {
                    row(Localized.wowpaySwapRefundTransaction, value: transactionId, copiesValue: true)
                }
            }
            .wowPayCard(padding: Spacing.wowPay.servicePadding, cornerRadius: Sizing.wowPay.serviceCorner)
        }
    }

    private func actions(_ order: BsvHomePortalSwapOrder) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.grid) {
            if order.canPay {
                StateButton(text: Localized.wowpaySwapPayOrder, type: .wowPayPrimary(model.operation == .paying ? .loading() : model.canPayOrder(order) ? .normal : .disabled)) {
                    Task { await model.payOrder(order) }
                }
                .disabled(!model.canPayOrder(order))
                .accessibilityIdentifier("wowpay-swap-pay-order")
            }
            if order.direction == nil {
                Text(Localized.wowpaySwapOrderContextUnavailable)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            if order.orderStatus == .completed, !order.payoutVerified {
                Button(Localized.wowpaySwapVerifyPayout) {
                    Task { await model.verifyPayout(order.orderId) }
                }
                .buttonStyle(.wowPaySecondary())
                .disabled(model.isBusy)
            }
            ForEach(order.requiredActions, id: \.actionId) { action in
                Button(WowPayPortalOrderLabels.action(action.actionType)) {
                    Task { await model.openAction(orderId: order.orderId, actionId: action.actionId) }
                }
                .buttonStyle(.wowPaySecondary())
                .disabled(model.isBusy)
            }
        }
    }

    private func row(_ title: String, value: String, copiesValue: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(title)
                .font(.wowPay.label)
            HStack(alignment: .top) {
                Text(value)
                    .font(.wowPay.rate)
                    .textSelection(.enabled)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if copiesValue {
                    Button {
                        CopyTypeViewModel.copyToClipboard(value, expirationTime: nil)
                        toast = ToastMessage(title: Localized.Common.copiedToClipboard, image: "doc.on.doc")
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                    }
                    .accessibilityLabel(Localized.Common.copy)
                    .buttonStyle(.plain)
                }
            }
        }
    }
}
