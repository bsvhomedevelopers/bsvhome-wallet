import Components
import struct BsvHomeCore.BsvHomeMpcTransfer
import struct BsvHomeCore.BsvHomeMpcWallet
import Localization
import Style
import SwiftUI

public struct MpcWalletScene: View {
    @Bindable private var model: MpcWalletViewModel
    private let initialWalletId: String?
    private let onScan: MpcRecipientInputAction?
    private let onContacts: MpcRecipientInputAction?
    @Binding private var isBalanceHidden: Bool
    @State private var destination: MpcWalletRoute?
    @State private var importing = false
    @State private var exportTicket: MpcBackupExportTicket?
    @State private var historyPage: UInt32 = 1
    @State private var qrImage: UIImage?

    public init(model: MpcWalletViewModel, initialWalletId: String? = nil, isBalanceHidden: Binding<Bool> = .constant(true), onScan: MpcRecipientInputAction? = nil, onContacts: MpcRecipientInputAction? = nil) {
        self.model = model
        self.initialWalletId = initialWalletId
        _isBalanceHidden = isBalanceHidden
        self.onScan = onScan
        self.onContacts = onContacts
    }

    public var body: some View {
        Group {
            if let initialWalletId {
                page(.wallet(initialWalletId))
            } else {
                walletList
            }
        }
        .task { await model.load() }
        .navigationDestination(for: MpcWalletRoute.self) { route in page(route) }
        .navigationDestination(item: $destination) { route in page(route) }
        .modifier(MpcBackupPresentation(model: model, ticket: $exportTicket))
    }

    private var walletList: some View {
        PhaseOnePage(title: Localized.phaseoneMpcWallets, isBusy: model.isBusy, error: model.error) {
            WowPayBrandHeader(title: Localized.wowpayBrand, subtitle: Localized.wowpayTagline, mark: Image("wowpay-mark")) { EmptyView() }
            if let selected = model.wallets.first(where: { $0.walletId == model.selectedWalletId }) ?? model.wallets.first {
                WowPayBalanceCard(title: selected.name, value: MpcWalletViewModel.bsv(selected.balance?.confirmedSatoshis), subtitle: selected.balance?.state ?? "—", isHidden: $isBalanceHidden, coinImage: Image("wowpay-bsv"))
                Text(selected.address).font(.wowPay.caption).textSelection(.enabled)
                HStack {
                    if onScan != nil {
                        Button(Localized.Wallet.scan) { recipientInput(selected.walletId, action: onScan, navigate: true) }
                    }
                    NavigationLink(Localized.Wallet.receive, value: MpcWalletRoute.receive(selected.walletId))
                    if selected.actions.contains(.send) {
                        NavigationLink(Localized.Wallet.send, value: MpcWalletRoute.send(selected.walletId))
                    }
                }.buttonStyle(.wowPaySecondary())
            }
            Text(Localized.phaseoneMpcHint).font(.wowPay.caption).wowPayCard()
            NavigationLink(Localized.phaseoneCreateMpc, value: MpcWalletRoute.create)
            NavigationLink(Localized.phaseoneRestoreMpc, value: MpcWalletRoute.restore)
            NavigationLink(Localized.phaseoneRecovery, value: MpcWalletRoute.recovery)
            if let pending = model.pendingTask {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(Localized.phaseoneOperationPending).font(.wowPay.rowTitle)
                    Text(pending.kind)
                    Text(String(describing: pending.state))
                    if let status = pending.status {
                        Text(status)
                    }
                    if let id = pending.id {
                        Text(id).textSelection(.enabled)
                    }
                    HStack {
                        if pending.actions.contains(.reconcile) {
                            Button(Localized.Common.refresh) { Task { await model.reconcile() } }
                        }
                        if pending.actions.contains(.cancel) {
                            Button(Localized.Common.cancel) { Task { await model.cancelTask() } }
                        }
                    }.buttonStyle(.wowPaySecondary()).disabled(model.isBusy)
                }.font(.wowPay.caption).wowPayCard()
            }
            if let pending = model.pendingPreparation {
                Button {
                    model.selectWallet(pending.walletId)
                    model.restorePendingDraft(pending.walletId)
                    destination = .send(pending.walletId)
                } label: { Text(Localized.phaseoneResumePayment + " · " + pending.recipient) }
            }
            ForEach(model.wallets, id: \.walletId) { wallet in
                NavigationLink(value: MpcWalletRoute.wallet(wallet.walletId)) {
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(wallet.name).font(.wowPay.rowTitle)
                        Text(wallet.address).font(.wowPay.caption)
                        Text(wallet.status).font(.wowPay.caption)
                        PrivacyText(MpcWalletViewModel.bsv(wallet.balance?.confirmedSatoshis), isEnabled: $isBalanceHidden)
                    }.frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
                }.buttonStyle(.plain)
            }
        }
        .refreshable { await model.load() }
    }

    private func page(_ route: MpcWalletRoute) -> some View {
        let backupWalletId: String? = if case let .backup(id) = route {
            id
        } else {
            nil
        }
        return PhaseOnePage(title: title(route), isBusy: model.isBusy, error: model.error) {
            switch route {
            case .create:
                Text(Localized.phaseoneMpcHint)
                TextField(Localized.phaseoneLabel, text: $model.name).wowPayInput()
                Button(Localized.phaseoneCreateMpc) {
                    Task {
                        if let id = await model.create() {
                            destination = .wallet(id)
                        }
                    }
                }.buttonStyle(.wowPay())
            case .restore:
                Text(Localized.phaseoneRestoreHint)
                TextField(Localized.phaseoneLabel, text: $model.name).wowPayInput()
                Button(Localized.phaseoneImportFile) { importing = true }.buttonStyle(.wowPaySecondary())
                SecureField(Localized.phaseoneRecoveryKey, text: $model.importRecoveryKey).textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                Button(Localized.phaseoneRestoreMpc) {
                    Task {
                        if let id = await model.restore() {
                            destination = .wallet(id)
                        }
                    }
                }.buttonStyle(.wowPay()).disabled(model.importedDocument == nil)
            case let .wallet(id):
                if let wallet = model.wallets.first(where: { $0.walletId == id }) {
                    overview(wallet)
                }
            case let .backup(id):
                backup(id)
            case let .receive(id):
                if let wallet = model.wallets.first(where: { $0.walletId == id }) {
                    receive(wallet)
                }
            case let .send(id):
                send(id)
            case let .transfer(id):
                if let transfer = model.transfer, transfer.id == id {
                    review(transfer)
                }
            case let .history(id):
                history(id)
            case let .paymail(id):
                paymail(id)
            case .recovery:
                recovery
            }
        }
        .disabled(model.isBusy)
        .task(id: route) { await load(route) }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
            if case let .success(url) = result {
                Task { await model.readBackup(url) }
            }
        }
        .modifier(MpcBackupPageLifetime(model: model, walletId: backupWalletId, ticket: $exportTicket))
    }

    private func overview(_ wallet: BsvHomeMpcWallet) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(wallet.name).font(.wowPay.sectionTitle)
            Text(wallet.address).textSelection(.enabled)
            Text(wallet.status).font(.wowPay.caption)
            PrivacyText(MpcWalletViewModel.bsv(wallet.balance?.confirmedSatoshis), isEnabled: $isBalanceHidden).font(.wowPay.amount)
            LabeledContent(Localized.phaseoneAvailable) {
                PrivacyText(MpcWalletViewModel.bsv(wallet.balance?.availableConfirmedSatoshis), isEnabled: $isBalanceHidden)
            }
            LabeledContent(Localized.phaseoneReserved) {
                PrivacyText(MpcWalletViewModel.bsv(wallet.balance?.reservedConfirmedSatoshis), isEnabled: $isBalanceHidden)
            }
            Text(wallet.balance?.state ?? "—").font(.wowPay.caption)
            Text(wallet.balance?.syncedAt ?? "—").font(.wowPay.caption)
            NavigationLink(Localized.Wallet.receive, value: MpcWalletRoute.receive(wallet.walletId))
            if wallet.actions.contains(.send) {
                NavigationLink(Localized.Wallet.send, value: MpcWalletRoute.send(wallet.walletId))
            }
            if wallet.actions.contains(.exportBackup) {
                NavigationLink(Localized.phaseoneBackup, value: MpcWalletRoute.backup(wallet.walletId))
            }
            NavigationLink(Localized.phaseoneHistory, value: MpcWalletRoute.history(wallet.walletId))
            NavigationLink(Localized.wowpayPaymail, value: MpcWalletRoute.paymail(wallet.walletId))
            if wallet.actions.contains(.startRecovery) {
                Button(Localized.phaseoneRecovery) { Task { await model.startRecovery(wallet.walletId); destination = .recovery } }
            }
            Button(Localized.Common.refresh) { Task { await model.refreshWallet(wallet.walletId) } }
        }
    }

    private func backup(_ id: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(Localized.phaseoneBackupHint)
            if let material = model.backup, material.walletId == id {
                Text(Localized.phaseoneRecoveryKey).font(.wowPay.rowTitle)
                Text(material.recoveryKey).font(.system(.body, design: .monospaced)).privacySensitive()
                Button(Localized.phaseoneSaveFile) {
                    Task {
                        if let ticket = await model.makeBackupExport() {
                            if model.isCurrentExport(ticket), !Task.isCancelled {
                                exportTicket = ticket
                            } else {
                                model.discardBackupExport(ticket)
                            }
                        }
                    }
                }.buttonStyle(.wowPaySecondary())
                SecureField(Localized.phaseoneRecoveryKeyConfirm, text: $model.recoveryKeyConfirmation)
                    .textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                Button(Localized.phaseoneConfirmBackup) { Task { await model.confirmBackup() } }
                    .buttonStyle(.wowPay()).disabled(model.savedCommitment != material.commitment)
            } else {
                Button(Localized.phaseoneBackup) { Task { await model.exportBackup(id) } }.buttonStyle(.wowPay())
            }
        }
    }

    private func receive(_ wallet: BsvHomeMpcWallet) -> some View {
        VStack(spacing: Spacing.wowPay.card) {
            if let qrImage {
                Image(uiImage: qrImage).resizable().interpolation(.none).scaledToFit()
                    .frame(width: Sizing.wowPay.receiveQR, height: Sizing.wowPay.receiveQR)
                    .background(.white)
            }
            Text(wallet.address).textSelection(.enabled)
            Button(Localized.Common.copy) { UIPasteboard.general.string = wallet.address }.buttonStyle(.wowPaySecondary())
            ShareLink(item: wallet.address).buttonStyle(.wowPaySecondary())
        }.frame(maxWidth: .infinity)
    }

    private func send(_ id: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            HStack {
                if onScan != nil {
                    Button(Localized.Wallet.scan) { recipientInput(id, action: onScan) }
                }
                if onContacts != nil {
                    Button(Localized.Contacts.title) { recipientInput(id, action: onContacts) }
                }
            }.buttonStyle(.wowPaySecondary())
            if let name = model.recipientName {
                Text(name).font(.wowPay.caption)
            }
            TextField(Localized.Transfer.Recipient.title, text: $model.recipient)
                .textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                .onChange(of: model.recipient) { model.draftChanged() }
            TextField("BSV", text: $model.amount).keyboardType(.decimalPad).wowPayInput()
                .onChange(of: model.amount) { model.draftChanged() }
            Button(Localized.phaseonePreparePayment) {
                Task {
                    if let transferId = await model.prepare(id) {
                        destination = .transfer(transferId)
                    }
                }
            }.buttonStyle(.wowPay())
            if model.needsPaymailSetup {
                NavigationLink(Localized.wowpayPaymail, value: MpcWalletRoute.paymail(id))
            }
        }
    }

    private func review(_ transfer: BsvHomeMpcTransfer) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            if let wallet = model.wallets.first(where: { $0.walletId == transfer.walletId }) {
                LabeledContent(Localized.Wallets.title, value: wallet.name)
                Text(wallet.address).font(.wowPay.caption).textSelection(.enabled)
            } else {
                Text(transfer.walletId).font(.wowPay.caption).textSelection(.enabled)
            }
            Text(transfer.recipientHandle ?? transfer.recipient).font(.wowPay.rowTitle).textSelection(.enabled)
            Text(transfer.recipient).font(.wowPay.caption).textSelection(.enabled)
            LabeledContent("BSV", value: MpcWalletViewModel.bsv(transfer.amount))
            LabeledContent(Localized.phaseoneFee, value: MpcWalletViewModel.bsv(transfer.fee))
            LabeledContent(Localized.phaseoneChange, value: MpcWalletViewModel.bsv(transfer.review.change))
            LabeledContent(Localized.phaseoneTotal, value: ((Decimal(transfer.amount) + Decimal(transfer.fee)) / Decimal(100_000_000)).formatted(.number.precision(.fractionLength(0 ... 8))) + " BSV")
            Text(transfer.status).font(.wowPay.rowTitle)
            Text(transfer.id).font(.wowPay.caption).textSelection(.enabled)
            if let transactionId = transfer.transactionId {
                Text(transactionId).font(.wowPay.caption).textSelection(.enabled)
                if let url = URL(string: "https://whatsonchain.com/tx/" + transactionId) {
                    Link(Localized.Activity.title, destination: url)
                }
            }
            if transfer.actions.contains(.sign) || transfer.actions.contains(.resumeSigning) {
                Button(Localized.phaseoneSignTransaction) { Task { await model.signTransfer() } }.buttonStyle(.wowPay())
            }
            if transfer.actions.contains(.broadcast) || transfer.actions.contains(.retryBroadcast) {
                if transfer.actions.contains(.retryBroadcast) {
                    Text(Localized.phaseonePaymentUnknown).font(.wowPay.caption)
                }
                Button(Localized.phaseoneBroadcastTransaction) { Task { await model.broadcastTransfer() } }.buttonStyle(.wowPay())
            }
            Button(Localized.Common.refresh) { Task { await model.refreshTransfer(transfer.id) } }.buttonStyle(.wowPaySecondary())
            if transfer.actions.contains(.cancel) {
                Button(Localized.Common.cancel, role: .destructive) { Task { await model.cancelTransfer() } }
            }
        }
    }

    private func history(_ id: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            if let history = model.history {
                ForEach(history.pendingTransfers, id: \.id) { transfer in
                    NavigationLink(value: MpcWalletRoute.transfer(transfer.id)) { Text(transfer.status + " · " + MpcWalletViewModel.bsv(transfer.amount)) }
                }
                ForEach(history.rows, id: \.id) { item in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text((item.incoming ? "+" : "−") + MpcWalletViewModel.bsv(item.amount))
                        Text(item.status).font(.wowPay.caption)
                        Text(item.transactionId).font(.wowPay.caption).textSelection(.enabled)
                        Text(item.source ?? "—").font(.wowPay.caption)
                    }.wowPayCard()
                }
                HStack {
                    Button(Localized.phaseonePreviousPage) { historyPage = max(1, historyPage - 1); Task { await model.loadHistory(id, page: historyPage) } }.disabled(historyPage <= 1)
                    Spacer()
                    Text(historyPage.formatted())
                    Spacer()
                    Button(Localized.Common.continue) { historyPage += 1; Task { await model.loadHistory(id, page: historyPage) } }
                }
            }
        }
    }

    private func paymail(_ id: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            if let snapshot = model.paymail {
                ForEach(snapshot.domains, id: \.self) { Text($0).font(.wowPay.caption) }
                ForEach(snapshot.bindings, id: \.handle) { binding in
                    Text(binding.handle).font(.wowPay.rowTitle)
                    Text(binding.address).font(.wowPay.caption).textSelection(.enabled)
                    Button(Localized.phaseoneRevoke, role: .destructive) {
                        model.handle = binding.handle
                        Task { await model.bindPaymail(id, active: false) }
                    }
                }
            }
            TextField(Localized.wowpayPaymail, text: $model.handle).textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
            Button(Localized.Common.continue) { Task { await model.bindPaymail(id, active: true) } }.buttonStyle(.wowPay())
        }
    }

    private var recovery: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(Localized.phaseoneRecoveryWait)
            if let recovery = model.recovery {
                Text(recovery.id).font(.wowPay.caption).textSelection(.enabled)
                Text(recovery.status).font(.wowPay.rowTitle)
                LabeledContent(Localized.phaseoneNotBefore, value: recovery.notBeforeAt ?? "—")
                Text(recovery.reviewNote ?? "—")
                Button(Localized.Common.refresh) { Task { await model.refreshRecovery() } }
                if recovery.actions.contains(.refresh) {
                    Button(Localized.Common.continue) { Task { await model.continueRecovery() } }.buttonStyle(.wowPay())
                }
                if recovery.actions.contains(.exportBackup) {
                    NavigationLink(Localized.phaseoneBackup, value: MpcWalletRoute.backup(recovery.walletId))
                }
                if recovery.actions.contains(.cancel) {
                    Button(Localized.Common.cancel, role: .destructive) { Task { await model.cancelRecovery() } }
                }
                if model.timeline?.canSupplyInformation == true {
                    TextField(Localized.phaseoneReason, text: $model.reason, axis: .vertical).wowPayInput()
                    Button(Localized.Common.continue) { Task { await model.supplyInformation() } }
                }
                if let timeline = model.timeline {
                    LabeledContent(Localized.Settings.Notifications.title, value: timeline.notificationStatus)
                    ForEach(Array(timeline.events.enumerated()), id: \.offset) { _, event in
                        VStack(alignment: .leading, spacing: Spacing.small) {
                            Text(event.eventType).font(.wowPay.rowTitle)
                            Text(event.time).font(.wowPay.caption)
                            if let description = event.description {
                                Text(description)
                            }
                            if let status = event.status {
                                Text(status).font(.wowPay.caption)
                            }
                            if let actorType = event.actorType {
                                Text(actorType).font(.wowPay.caption)
                            }
                        }.wowPayCard()
                    }
                }
            }
        }
    }

    private func load(_ route: MpcWalletRoute) async {
        guard !Task.isCancelled else { return }
        switch route {
        case let .wallet(id), let .backup(id), let .receive(id), let .send(id), let .history(id), let .paymail(id): model.selectWallet(id)
        case .create, .restore, .transfer, .recovery: break
        }
        switch route {
        case let .transfer(id): await model.refreshTransfer(id)
        case let .history(id): historyPage = 1; await model.loadHistory(id, page: 1)
        case let .paymail(id): await model.loadPaymail(id)
        case .recovery: await model.loadRecovery(); await model.refreshRecovery()
        case let .receive(id):
            if let address = model.wallets.first(where: { $0.walletId == id })?.address {
                qrImage = await QRCodeGenerator().generate(from: address, size: CGSize(width: Sizing.wowPay.receiveQR, height: Sizing.wowPay.receiveQR))
            }
        default: break
        }
    }

    private func recipientInput(_ walletId: String, action: MpcRecipientInputAction?, navigate: Bool = false) {
        model.selectWallet(walletId)
        guard let action, let apply = model.recipientInputCallback(walletId: walletId) else { return }
        action { payment in
            if apply(payment), navigate {
                destination = .send(walletId)
            }
        }
    }

    private func title(_ route: MpcWalletRoute) -> String {
        switch route {
        case .create: Localized.phaseoneCreateMpc
        case .restore: Localized.phaseoneRestoreMpc
        case .wallet: Localized.phaseoneMpcWallets
        case .backup: Localized.phaseoneBackup
        case .receive: Localized.Wallet.receive
        case .send, .transfer: Localized.Wallet.send
        case .history: Localized.phaseoneHistory
        case .paymail: Localized.wowpayPaymail
        case .recovery: Localized.phaseoneRecovery
        }
    }
}
