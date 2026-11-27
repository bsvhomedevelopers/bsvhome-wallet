import Components
import struct BsvHomeCore.BsvHomeSendContactRecipient
import struct BsvHomeCore.BsvHomeSendWalletRecipient
import Localization
import Style
import SwiftUI

public struct ContactRecipientPicker: View {
    private let recipients: [BsvHomeSendContactRecipient]
    private let onSelect: (BsvHomeSendContactRecipient) -> Void
    private let onClose: () -> Void
    private let walletRecipients: [BsvHomeSendWalletRecipient]
    private let onSelectWallet: ((BsvHomeSendWalletRecipient) -> Void)?
    private let walletRecipientsState: StateViewType<Void>
    private let onRetryWallets: (() -> Void)?

    public init(
        recipients: [BsvHomeSendContactRecipient],
        onSelect: @escaping (BsvHomeSendContactRecipient) -> Void,
        onClose: @escaping () -> Void,
        walletRecipients: [BsvHomeSendWalletRecipient] = [],
        onSelectWallet: ((BsvHomeSendWalletRecipient) -> Void)? = nil,
        walletRecipientsState: StateViewType<Void> = .data(()),
        onRetryWallets: (() -> Void)? = nil,
    ) {
        self.recipients = recipients
        self.onSelect = onSelect
        self.onClose = onClose
        self.walletRecipients = walletRecipients
        self.onSelectWallet = onSelectWallet
        self.walletRecipientsState = walletRecipientsState
        self.onRetryWallets = onRetryWallets
    }

    public var body: some View {
        NavigationStack {
            Group {
                if isEmpty {
                    ContentUnavailableView(onSelectWallet == nil ? Localized.Contacts.State.Empty.title : Localized.Common.noResultsFound, systemImage: "person.crop.circle")
                } else {
                    List {
                        if let onSelectWallet {
                            if !recipients.isEmpty {
                                Section(Localized.Contacts.title) { contactRows }
                            }
                            Section(Localized.Wallets.title) {
                                if walletRecipientsState.isLoading || walletRecipientsState.isNoData {
                                    LoadingView()
                                }
                                if case .error = walletRecipientsState {
                                    Text(Localized.Errors.errorOccurred)
                                        .foregroundStyle(Colors.WowPay.secondaryText)
                                    if let onRetryWallets {
                                        Button(Localized.Common.tryAgain, action: onRetryWallets)
                                    }
                                }
                                ForEach(walletRecipients, id: \.walletId) { item in
                                    Button { onSelectWallet(item) } label: {
                                        recipientRow(name: item.recipient.name, address: item.recipient.address)
                                    }
                                    .buttonStyle(.plain)
                                    .disabled(walletRecipientsState.isLoading || walletRecipientsState.isError)
                                    .accessibilityIdentifier("sendRecipientWallet_" + item.walletId)
                                }
                            }
                        } else {
                            contactRows
                        }
                    }
                }
            }
            .navigationTitle(onSelectWallet == nil ? Localized.Contacts.title : Localized.Transfer.Recipient.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(Localized.Common.done, action: onClose)
                }
            }
        }
    }

    private var isEmpty: Bool {
        recipients.isEmpty && (onSelectWallet == nil || (walletRecipients.isEmpty && walletRecipientsState.value != nil))
    }

    private var contactRows: some View {
        ForEach(recipients, id: \.id) { item in
            Button { onSelect(item) } label: {
                recipientRow(name: item.recipient.name, address: item.recipient.address)
            }
            .buttonStyle(.plain)
        }
    }

    private func recipientRow(name: String?, address: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.tiny) {
            Text(name ?? address)
                .foregroundStyle(Colors.WowPay.heading)
            Text(address)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
}
