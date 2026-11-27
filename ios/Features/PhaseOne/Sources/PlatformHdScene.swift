import Components
import struct BsvHomeCore.BsvHomePlatformHdAccount
import Localization
import Style
import SwiftUI

public struct PlatformHdScene: View {
    @Bindable private var model: PlatformHdViewModel

    public init(model: PlatformHdViewModel) {
        self.model = model
    }

    public var body: some View {
        PhaseOnePage(title: Localized.phaseoneHd, isBusy: model.isBusy, error: model.error) {
            Text(Localized.phaseoneHdHint).font(.wowPay.caption)
            walletPicker
            NavigationLink { enrollment } label: { Label(Localized.phaseoneHdEnroll, systemImage: "plus.circle") }
                .disabled(model.wallets.isEmpty)
            if model.accounts.isEmpty {
                Text(Localized.phaseoneHdEmpty).font(.wowPay.body)
            }
            ForEach(model.accounts, id: \.id) { account in
                NavigationLink { details(account) } label: {
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(account.label).font(.wowPay.rowTitle)
                        Text(account.basePath).font(.wowPay.caption)
                    }.frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
                }
            }
            Button(Localized.phaseoneRefresh) { Task { await model.load() } }.buttonStyle(.wowPaySecondary())
        }
        .disabled(model.isBusy)
        .task { await model.load() }
        .refreshable { await model.load() }
        .onDisappear { model.consent = false }
    }

    private var walletPicker: some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(Localized.phaseoneHdLocalWallet).font(.wowPay.rowTitle)
            if model.wallets.isEmpty {
                Text(Localized.phaseoneHdSelectWallet).font(.wowPay.caption)
            }
            Picker(Localized.phaseoneHdLocalWallet, selection: Binding(get: { model.localWalletId }, set: { model.selectLocal($0) })) {
                ForEach(model.wallets, id: \.id) { wallet in Text(wallet.name).tag(Optional(wallet.id)) }
            }.pickerStyle(.menu)
            if let wallet = model.wallets.first(where: { $0.id == model.localWalletId }) {
                Text(wallet.basePath).font(.wowPay.caption)
                Text(wallet.firstAddress).font(.wowPay.caption).textSelection(.enabled)
            }
        }.wowPayCard()
    }

    private var enrollment: some View {
        PhaseOnePage(title: Localized.phaseoneHdEnroll, isBusy: model.isBusy, error: model.error) {
            walletPicker
            TextField(Localized.phaseoneLabel, text: $model.label).wowPayInput()
            Toggle(Localized.phaseoneConsentXpub, isOn: $model.consent)
            Text(Localized.phaseoneHdHint).font(.wowPay.caption)
            Button(Localized.phaseoneHdEnroll) { Task { await model.enroll() } }
                .buttonStyle(.wowPay()).disabled(!model.consent || model.localWalletId == nil)
        }.disabled(model.isBusy)
            .onDisappear { model.cancel() }
    }

    private func details(_ account: BsvHomePlatformHdAccount) -> some View {
        PhaseOnePage(title: account.label, isBusy: model.isBusy, error: model.error) {
            Text(account.basePath).font(.wowPay.caption)
            if let snapshot = model.snapshot, snapshot.account.id == account.id {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(Localized.phaseoneStatus + ": " + status(snapshot.balance.state))
                    Text(Localized.phaseoneHdConfirmed + ": " + (snapshot.balance.confirmedSatoshis ?? "—") + " " + Localized.phaseoneUnitSatoshi)
                    Text(Localized.phaseoneUnconfirmed + ": " + (snapshot.balance.unconfirmedSatoshis ?? "—") + " " + Localized.phaseoneUnitSatoshi)
                    Text(Localized.phaseoneHdUnverified + ": " + String(snapshot.unverifiedAddressCount)).font(.wowPay.caption)
                }.wowPayCard()
                ForEach(snapshot.addresses, id: \.walletId) { address in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(Localized.phaseoneHdAddressIndex + ": " + String(address.index)).font(.wowPay.caption)
                        Text(address.address).textSelection(.enabled)
                    }.wowPayCard()
                }
                Button(Localized.phaseoneHdAllocate) { Task { await model.allocate(account.id) } }
                    .buttonStyle(.wowPay()).disabled(!snapshot.canAllocate)
            }
            Button(Localized.phaseoneRefresh) { Task { await model.refresh(account.id) } }.buttonStyle(.wowPaySecondary())
            scanControls(account.id)
        }.disabled(model.isBusy)
            .task { await model.open(account.id) }
            .onDisappear { model.cancel() }
    }

    private func scanControls(_ id: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            Text(Localized.phaseoneScan).font(.wowPay.rowTitle)
            Text(Localized.phaseoneHdScanHint).font(.wowPay.caption)
            TextField(Localized.phaseoneHdGapLimit, text: $model.gapLimit).keyboardType(.numberPad).wowPayInput()
            TextField(Localized.phaseoneHdMaxIndex, text: $model.maximumIndex).keyboardType(.numberPad).wowPayInput()
            if let scan = model.scan {
                Text(status(scan.status))
                Text(Localized.phaseoneHdAddressIndex + ": " + String(scan.nextIndex)).font(.wowPay.caption)
                if let error = scan.errorCode {
                    Text(error).font(.wowPay.caption)
                }
                Button(Localized.phaseoneRefresh) { Task { await model.refreshScan() } }.buttonStyle(.wowPaySecondary())
                if scan.actions.contains(.cancel) {
                    Button(Localized.Common.cancel) { Task { await model.cancelScan() } }.buttonStyle(.wowPaySecondary())
                }
                if scan.actions.contains(.resume) {
                    Button(Localized.Common.continue) { Task { await model.resumeScan() } }.buttonStyle(.wowPay())
                }
            } else {
                Button(Localized.phaseoneScan) { Task { await model.startScan(id) } }.buttonStyle(.wowPaySecondary())
            }
        }.wowPayCard()
    }

    private func status(_ code: String) -> String {
        switch code {
        case "CURRENT": Localized.phaseoneStateCurrent
        case "COMPLETE": Localized.phaseoneStateComplete
        case "QUEUED", "RUNNING", "SYNCING": Localized.phaseoneStateRunning
        case "CANCELLED": Localized.phaseoneStateCancelled
        case "LIMIT_REACHED": Localized.phaseoneHdLimitReached
        case "ERROR", "RETRY": Localized.phaseoneStateError
        case "STALE": Localized.phaseoneStateStale
        default: Localized.phaseoneStateUnknown
        }
    }
}
