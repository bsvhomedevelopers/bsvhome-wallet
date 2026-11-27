import Components
import Localization
import Style
import SwiftUI

public struct WowPayHomeContent: View {
    private let header: AnyView
    private let balance: AnyView
    private let operations: AnyView?
    private let status: AnyView?
    private let isReceiveEnabled: Bool
    private let isSendEnabled: Bool
    private let onSelectScan: () -> Void
    private let onSelectReceive: () -> Void
    private let onSelectSend: () -> Void

    public init(
        header: AnyView,
        balance: AnyView,
        isReceiveEnabled: Bool,
        isSendEnabled: Bool,
        onSelectScan: @escaping () -> Void,
        onSelectReceive: @escaping () -> Void,
        onSelectSend: @escaping () -> Void,
        operations: AnyView? = nil,
        status: AnyView? = nil,
    ) {
        self.header = header
        self.balance = balance
        self.isReceiveEnabled = isReceiveEnabled
        self.isSendEnabled = isSendEnabled
        self.onSelectScan = onSelectScan
        self.onSelectReceive = onSelectReceive
        self.onSelectSend = onSelectSend
        self.operations = operations
        self.status = status
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                VStack(spacing: .zero) {
                    VStack(spacing: Spacing.space8) {
                        header
                        VStack(spacing: Spacing.wowPay.rowPadding) {
                            balance
                            quickActions
                        }
                    }
                    if let operations {
                        operations.padding(.top, Spacing.wowPay.homeOperations)
                    }
                }
                if let status {
                    status
                }
            }
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.bottom, Spacing.space24)
        }
        .wowPayBackground(.solid)
    }

    private var quickActions: some View {
        HStack(alignment: .top, spacing: Spacing.wowPay.grid) {
            WowPayQuickActionView(title: Localized.Wallet.scan, image: Image("wowpay-scan"), action: onSelectScan)
                .accessibilityIdentifier("scan")
            WowPayQuickActionView(title: Localized.Wallet.receive, image: Image("wowpay-qr-code"), action: onSelectReceive)
                .accessibilityIdentifier("receive_button")
                .disabled(!isReceiveEnabled)
            WowPayQuickActionView(title: Localized.Wallet.send, image: Image("wowpay-transfer"), action: onSelectSend)
                .accessibilityIdentifier("send_button")
                .disabled(!isSendEnabled)
        }
    }
}
