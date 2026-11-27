public import class Components.NavigationPathState
import BigInt
import struct Components.WowPayBalanceCard
import Formatters
import Foundation
import BsvHomeCore
import Localization
import Primitives
import PrimitivesComponents
import QRScanner
import Style
import SwiftUI

public struct BsvColdWalletScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var lifetime: BsvColdWalletLifetime?
    private let model: BsvColdWalletViewModel
    private let navigationOwner: NavigationPathState
    private let onScanRecipient: BsvColdWalletRecipientScanner?
    private let onRequestSigning: (@MainActor (String) -> Void)?
    private let onRequestPairing: (@MainActor (String) -> Void)?
    private let onInitialPageBack: (@MainActor () -> Void)?
    private let dashboard: (@MainActor (AnyView, AnyView, AnyView?) -> AnyView)?
    private let navigation: BsvColdWalletNavigation

    public init(model: BsvColdWalletViewModel, navigation: NavigationPathState, onScanRecipient: BsvColdWalletRecipientScanner? = nil, dashboard: (@MainActor (AnyView, AnyView, AnyView?) -> AnyView)? = nil, onInitialPageBack: (@MainActor () -> Void)? = nil, onRequestSigning: (@MainActor (String) -> Void)? = nil, onRequestPairing: (@MainActor (String) -> Void)? = nil) {
        self.model = model
        navigationOwner = navigation
        self.onScanRecipient = onScanRecipient
        self.onInitialPageBack = onInitialPageBack
        self.onRequestSigning = onRequestSigning
        self.onRequestPairing = onRequestPairing
        self.dashboard = dashboard
        self.navigation = model.navigation(in: navigation)
    }

    public var body: some View {
        @Bindable var model = model
        rootPage
            .sheet(item: scannerBinding) { session in
                ScanQRCodeNavigationStack(scanType: .universal, mode: .continuous, hint: scanHint, allowsPhotoLibrary: !model.isOffline) {
                    model.scan($0, scannerId: session.id)
                }
                .interactiveDismissDisabled(model.isBusy)
            }
            .alert(Localized.coldWalletScanRequest, isPresented: Binding(get: { model.replacementRequest != nil }, set: {
                if !$0 {
                    model.replacementRequest = nil
                }
            }), presenting: model.replacementRequest) { request in
                Button(Localized.Common.continue) { model.confirmReplacement(request) }
                Button(Localized.Common.cancel, role: .cancel) { model.replacementRequest = nil }
            } message: { _ in
                Text(Localized.coldWalletReplaceSignedHint)
            }
            .task { await model.load() }
            .onChange(of: scenePhase, initial: true) { _, phase in
                model.sceneChanged(phase)
                if phase == .active, navigation.isShowing(.signing, model: model) {
                    model.activateInitialRequestScanner()
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in updateCapture() }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataWillBecomeUnavailableNotification)) { _ in model.cancelActions() }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.userDidTakeScreenshotNotification)) { _ in model.cancelActions() }
            .onAppear {
                if lifetime == nil {
                    lifetime = BsvColdWalletLifetime(model: model)
                }
                updateCapture()
            }
    }

    func destination(_ destination: BsvColdWalletPageRoute) -> some View {
        page(destination.page).navigationTitle(title(destination.page))
            .toolbar(.visible, for: .navigationBar)
            .onAppear { reconcileDestination(destination) }
            .onChange(of: navigationOwner.path) { _, _ in reconcileDestination(destination) }
    }

    private func reconcileDestination(_ destination: BsvColdWalletPageRoute) {
        guard navigation.owns(destination, model: model, navigation: navigationOwner) else { return }
        navigation.reconcile(model: model)
        if destination.page == .signing, navigation.isShowing(.signing, model: model) {
            model.activateInitialRequestScanner()
        }
    }

    @ViewBuilder
    private var rootPage: some View {
        if let dashboard, model.mode == .unified, model.isSigner {
            dashboard(AnyView(dashboardBalance), AnyView(dashboardOperations), dashboardStatus)
                .toolbar(.hidden, for: .navigationBar)
                .interactiveDismissDisabled(model.isBusy)
                .overlay { privacyCover }
        } else {
            page(nil).navigationTitle(Localized.coldWalletTitle)
        }
    }

    private var dashboardBalance: some View {
        WowPayBalanceCard(title: Localized.coldWalletTitle, value: "— BSV", subtitle: Localized.coldWalletSigning, isHidden: .constant(false), visibilityControl: .hidden)
            .accessibilityIdentifier("coldWalletDashboardBalance")
    }

    private var dashboardOperations: some View {
        VStack(spacing: Spacing.medium) {
            primary(Localized.coldWalletScanRequest, enabled: !model.storageProtectionFailed && onRequestSigning != nil, action: { model.requestSigning(using: onRequestSigning) })
                .accessibilityIdentifier("coldWalletDashboardSigning")
            Button {
                guard let onRequestPairing else { return }
                model.requestPairing(using: onRequestPairing)
            } label: {
                HStack(spacing: Spacing.space8) {
                    Image("wowpay-key").renderingMode(.template).resizable().scaledToFit()
                        .frame(width: Sizing.wowPay.coldWalletServiceIcon, height: Sizing.wowPay.coldWalletServiceIcon)
                        .foregroundStyle(Colors.WowPay.brand)
                        .frame(width: Sizing.wowPay.coldWalletServiceAvatar, height: Sizing.wowPay.coldWalletServiceAvatar)
                        .background(Colors.WowPay.pale, in: RoundedRectangle(cornerRadius: Sizing.wowPay.serviceIconCorner))
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: Spacing.space4) {
                        Text(Localized.coldWalletPair).font(.wowPay.label).foregroundStyle(Colors.WowPay.heading).lineLimit(1)
                        Text(Localized.coldWalletPairingHint).font(.wowPay.coldWalletServiceSubtitle)
                            .foregroundStyle(Colors.WowPay.secondaryText).lineSpacing(Spacing.space2).lineLimit(2)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.horizontal, Spacing.space12)
                .padding(.vertical, Spacing.wowPay.coldWalletServiceVertical)
                .contentShape(Rectangle())
                .wowPayCard(padding: .zero)
            }
            .buttonStyle(.plain)
            .disabled(model.isBusy || onRequestPairing == nil)
            .accessibilityLabel(Localized.coldWalletPair)
            .accessibilityIdentifier("coldWalletDashboardPairing")
        }
    }

    private var dashboardStatus: AnyView? {
        AnyView(VStack(alignment: .leading, spacing: Spacing.wowPay.coldWalletPage) {
            notice(Localized.coldWalletOfflineNetworkHint)
            if let error = model.errorMessage {
                notice(error, warning: true)
            }
            if model.loadFailed, !model.storageProtectionFailed {
                primary(Localized.Common.tryAgain, action: model.retryLoad)
                    .accessibilityIdentifier("coldWalletRetryLoad")
            }
            if model.isBusy {
                ProgressView().frame(maxWidth: .infinity)
            }
            if let error = model.backupError {
                notice(error, warning: true)
            }
        })
    }

    @ViewBuilder
    private var privacyCover: some View {
        if !model.isForeground || model.isCaptured {
            Colors.WowPay.surface.ignoresSafeArea()
                .overlay { Text(Localized.SecretPhrase.ContentHidden.description).padding() }
                .accessibilityIdentifier("coldWalletPrivacyCover")
        }
    }

    private var scannerBinding: Binding<BsvColdWalletScanner?> {
        let id = model.scannerSession?.id
        return Binding(get: { model.scannerSession }, set: { value in
            if value == nil, let id, model.closeScanner(id) == .initialEntry {
                navigation.cancelInitialScanner(model: model, onInitialPageBack: onInitialPageBack)
            }
        })
    }

    private var scanHint: String {
        let progress = Localized.coldWalletQrProgress(model.receivedFrames)
        return model.errorMessage.map { progress + "\n" + $0 } ?? progress
    }

    private func page(_ route: BsvColdWalletRoute?) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.coldWalletPage) {
                if let hint = pageHint(route) {
                    Text(hint)
                        .font(.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .wowPayCard()
                }
                if (model.isOffline && route == nil) || (model.mode == .unified && model.isSigner) {
                    notice(Localized.coldWalletOfflineNetworkHint)
                }
                if let error = model.errorMessage {
                    notice(error, warning: true)
                }
                if model.loadFailed, !model.storageProtectionFailed {
                    primary(Localized.Common.tryAgain, action: model.retryLoad)
                        .accessibilityIdentifier("coldWalletRetryLoad")
                }
                if model.isBusy {
                    ProgressView().frame(maxWidth: .infinity)
                }
                content(route)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.bottom, Spacing.wowPay.coldWalletPageBottom)
        }
        .wowPayBackground(.solid)
        .scrollDismissesKeyboard(.interactively)
        .navigationBarTitleDisplayMode(.inline)
        .interactiveDismissDisabled(model.isBusy)
        .overlay { privacyCover }
    }

    private func pageHint(_ route: BsvColdWalletRoute?) -> String? {
        guard model.mode == .unified else {
            return model.isOffline ? Localized.coldWalletOfflineHint : Localized.coldWalletOnlineHint
        }
        return switch route {
        case nil: model.isSigner ? nil : Localized.coldWalletOnlineHint
        case .workspaceHome, .overview, .send, .receive: Localized.coldWalletOnlineHint
        case .signing, .review, .signed: Localized.coldWalletOfflineHint
        case .create, .recover: Localized.coldWalletUnifiedHint
        case .connect, .importReview, .pairing, .request, .broadcast, .result, .backup: nil
        }
    }

    @ViewBuilder
    private func content(_ route: BsvColdWalletRoute?) -> some View {
        switch route {
        case nil: home
        case .signing: signingHome
        case .workspaceHome: home
        case .create: creation
        case .recover, .connect: importForm
        case .importReview: importReview
        case .pairing: pairing
        case .overview: overview
        case .receive: receive
        case .send: send
        case .request: request
        case .review: signingReview
        case .signed: signed
        case .broadcast: broadcast
        case .result: result
        case .backup: backup
        }
    }

    private var home: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            walletSelector
            if model.storageProtectionFailed {
                notice(Localized.coldWalletStorageProtectionFailed, warning: true)
                primary(Localized.Common.tryAgain, action: model.retryStorageProtection)
            } else if model.mode == .offline {
                if model.isSigner {
                    primary(Localized.coldWalletScanRequest, action: { model.startScan(.request) })
                    secondary(Localized.coldWalletPair, action: model.exportPairing)
                }
                primary(Localized.coldWalletCreate, action: model.generateWords)
                secondary(Localized.coldWalletRecover, action: { model.open(.recover) })
            } else if model.isObserver {
                primary(Localized.coldWalletObserver, action: { model.open(.overview) })
            }
            if model.mode == .online {
                secondary(Localized.coldWalletConnect, action: { model.open(.connect) })
            }
            if model.pending != nil, model.mode != .unified || model.isObserver {
                secondary(Localized.coldWalletPending, action: model.showPending)
            }
        }
    }

    private var signingHome: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            walletSelector
            primary(Localized.coldWalletScanRequest, enabled: model.isSigner, action: { model.startScan(.request) })
                .accessibilityIdentifier("coldWalletWorkspaceScanRequest")
            if model.entryIsPairing {
                secondary(Localized.coldWalletPair, enabled: model.isSigner, action: model.exportPairing)
            }
            if model.pending != nil {
                secondary(Localized.coldWalletPending, action: model.showPending)
            }
        }
    }

    private var walletSelector: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(Localized.coldWalletChoose).font(.title2.weight(.heavy))
            if model.mode != .unified || !model.selectionCandidates.isEmpty {
                Text(Localized.coldWalletChooseHint).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            }
            if model.selectionCandidates.isEmpty {
                if model.mode == .unified {
                    Text(Localized.coldWalletSetupHint).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                } else {
                    notice(Localized.Errors.noDataAvailable)
                }
            }
            ForEach(model.selectionCandidates, id: \.wallet.id) { candidate in
                let wallet = candidate.wallet
                Button { model.selectWallet(wallet.id) } label: {
                    HStack(spacing: Spacing.space12) {
                        Image(systemName: candidate.role == .observer ? "eye" : "wallet.pass")
                            .foregroundStyle(Colors.WowPay.brand)
                        VStack(alignment: .leading, spacing: Spacing.space6) {
                            Text(wallet.name).font(.callout.bold())
                            Text(candidate.role == .observer ? Localized.coldWalletWatchOnly : Localized.coldWalletLocalKeys).font(.caption)
                            Text(firstAddress(wallet)).font(.caption.monospaced()).lineLimit(1).truncationMode(.middle)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: wallet.id == model.wallet?.id ? "largecircle.fill.circle" : "circle")
                    }
                    .foregroundStyle(Colors.WowPay.heading)
                    .padding(Spacing.wowPay.rowPadding)
                    .contentShape(Rectangle())
                    .wowPayListCard(isSelected: wallet.id == model.wallet?.id)
                }
                .buttonStyle(.plain).disabled(model.isBusy)
                .accessibilityIdentifier("coldWalletChoice-" + wallet.id)
            }
        }
    }

    private var creation: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            notice(Localized.coldWalletBackupHint, warning: true)
            nameField
            SecretDataTypeView(type: .words(words: WordIndex.rows(for: model.words)), style: .wowPay)
                .privacySensitive()
            Toggle(Localized.coldWalletBackupConfirm, isOn: $model.isBackupConfirmed)
                .disabled(model.isBusy)
            primary(Localized.coldWalletCreate, enabled: model.isBackupConfirmed && !model.walletName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.walletName.utf16.count <= 30, action: model.create)
        }
    }

    private var importForm: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            notice(model.isSecretImport ? Localized.bsvHdBackupHint : Localized.coldWalletPairingHint, warning: model.isSecretImport)
            nameField
            if model.isSecretImport {
                SecureField(Localized.bsvHdImportInput, text: $model.input)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                    .privacySensitive().wowPayInput()
            } else {
                TextField(Localized.bsvHdPublicInput, text: $model.input, axis: .vertical)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                    .lineLimit(4 ... 8).privacySensitive().wowPayInput()
            }
            BsvHdImportSettings(configuration: $model.configuration, passphrase: $model.passphrase, isSecret: model.isSecretImport)
            if !model.isSecretImport {
                secondary(Localized.Wallet.scan, enabled: !model.walletName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.configuration.isComplete, action: { model.startScan(.pairing) })
            }
            primary(Localized.Common.continue, enabled: model.canImport, action: model.previewImport)
        }
        .disabled(model.isBusy)
    }

    private var importReview: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            BsvHdImportReview(accountPath: model.previewPath, firstAddress: model.previewAddress, isConfirmed: $model.isPreviewConfirmed)
            primary(Localized.Wallet.Import.action, enabled: model.isPreviewConfirmed, action: model.confirmImport)
        }
        .disabled(model.isBusy)
    }

    private var pairing: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.coldWalletPage) {
            notice(Localized.coldWalletPairingHint)
            VStack(alignment: .leading, spacing: Spacing.space10) {
                pairingValue(Localized.Wallet.name, model.wallet?.name ?? "")
                pairingValue(Localized.Common.address, model.wallet.map(firstAddress) ?? "")
            }
            .wowPayCard()
            .accessibilityIdentifier("coldWalletPairingIdentity")
            BsvColdWalletQRPlayer(frames: model.frames)
            primary(Localized.Common.done, action: model.complete)
        }
    }

    private func pairingValue(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space4) {
            Text(title).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
            Text(value).font(.wowPay.coldWalletValue).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, Spacing.space4)
    }

    private var overview: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            value(Localized.Wallet.name, model.wallet?.name ?? "")
            Text(model.balance).font(.wowPay.onboardingTitle)
            primary(Localized.Wallet.send, enabled: model.isObserver, action: { model.open(.send) })
            secondary(Localized.Wallet.receive, enabled: model.addresses != nil, action: { model.open(.receive) })
            secondary(Localized.multisigRefresh, action: model.refresh)
            if model.pending != nil {
                secondary(Localized.coldWalletPending, action: model.showPending)
            }
            Text(Localized.Activity.title).font(.title2.weight(.heavy))
            ForEach(model.snapshot?.transactions ?? [], id: \.id) { transaction in
                VStack(alignment: .leading, spacing: Spacing.space6) {
                    Text(transaction.id).font(.caption).lineLimit(2)
                    if let value = UInt64(exactly: transaction.value) {
                        Text((transaction.direction == .outgoing ? "−" : transaction.direction == .incoming ? "+" : "") + BsvColdWalletAmount.display(value) + " BSV")
                    }
                    Text(transaction.direction == .outgoing ? transaction.to : transaction.from).font(.caption)
                    Text(transaction.createdAt.formatted(.dateTime.month().day().hour().minute())).font(.caption)
                }
                .frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
            }
        }
    }

    private var receive: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            notice(Localized.coldWalletReceiveHint)
            if model.receiveAddress.isEmpty {
                notice(Localized.wowpayAddressDescription)
            } else {
                BsvColdWalletQRPlayer(frames: [model.receiveAddress])
                value(Localized.Common.address, model.receiveAddress)
                secondary(Localized.Common.copy, action: { copy(model.receiveAddress) })
            }
            secondary(Localized.wowpayAddressAdd, enabled: model.addresses?.canAdd == true, action: model.addAddress)
            ForEach(model.addresses?.addresses ?? [], id: \.index) { address in
                VStack(alignment: .leading, spacing: Spacing.space6) {
                    HStack {
                        Button { model.selectAddress(address.index) } label: {
                            Label(Localized.wowpayBsvReceivingAddress + " " + String(Int(address.index) + 1), systemImage: address.index == model.addresses?.selectedIndex ? "checkmark.circle.fill" : "circle")
                        }
                        .disabled(model.isBusy)
                        Spacer()
                        Button(Localized.Common.copy) { copy(address.address) }
                    }
                    Text(address.address).font(.caption).textSelection(.enabled)
                }
                .wowPayCard()
            }
        }
    }

    private var send: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            if !model.isObserver || model.isOffline {
                notice(Localized.coldWalletConnectRequired, warning: true)
            } else if model.pending?.broadcasted == false {
                notice(Localized.coldWalletPendingHint)
                primary(Localized.coldWalletPending, action: model.showPending)
                if model.pending?.signedJson == nil {
                    secondary(Localized.Common.cancel, action: model.cancelRequest)
                }
            } else {
                TextField(Localized.Common.address, text: $model.recipient, axis: .vertical)
                    .textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                if let onScanRecipient {
                    secondary(Localized.wowpaySendScanQr, action: { model.scanRecipient(using: onScanRecipient) })
                        .accessibilityIdentifier("coldWalletScanRecipient")
                }
                TextField(Localized.wowpayAmount, text: $model.amount)
                    .keyboardType(.decimalPad).wowPayInput()
                primary(Localized.coldWalletCreateRequest, enabled: model.canPrepare && !model.recipient.isEmpty && BsvColdWalletAmount.satoshis(model.amount, decimalSeparator: Locale.current.decimalSeparator ?? ".") != nil, action: { model.prepare(navigation: navigationOwner) })
            }
        }
        .disabled(model.isBusy)
    }

    private var request: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            reviewDetails
            notice(Localized.coldWalletRequestHint)
            BsvColdWalletQRPlayer(frames: model.frames)
            primary(Localized.coldWalletScanSigned, action: { model.startScan(.signed) })
            if model.pending?.signedJson == nil {
                secondary(Localized.Common.cancel, action: model.cancelRequest)
            }
        }
    }

    private var signingReview: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            reviewDetails
            notice(Localized.coldWalletReviewHint, warning: true)
            Toggle(Localized.coldWalletReviewConfirm, isOn: $model.isReviewConfirmed).disabled(model.isBusy)
            primary(Localized.multisigAuthenticateSign, enabled: model.canSign, action: { model.sign(navigation: navigationOwner) })
            secondary(Localized.Common.cancel, action: model.cancelRequest)
        }
    }

    private var signed: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            reviewDetails
            value(Localized.coldWalletTransactionId, model.transactionId)
            notice(Localized.coldWalletSignedHint)
            BsvColdWalletQRPlayer(frames: model.frames)
            primary(Localized.Wallet.backToAssets, action: model.complete)
        }
    }

    private var broadcast: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            reviewDetails
            value(Localized.coldWalletTransactionId, model.transactionId)
            notice(Localized.coldWalletBroadcastHint, warning: true)
            primary(Localized.multisigBroadcast, enabled: model.canBroadcast, action: { model.broadcast(navigation: navigationOwner) })
        }
    }

    private var result: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            notice(Localized.coldWalletBroadcastSuccess)
            if model.hasPersistenceWarning {
                notice(Localized.coldWalletBroadcastStorageFailed, warning: true)
            }
            reviewDetails
            value(Localized.coldWalletTransactionId, model.transactionId)
            secondary(Localized.Common.copy, action: { copy(model.transactionId) })
            primary(Localized.multisigRefresh, action: model.refresh)
            secondary(Localized.Wallet.backToAssets, action: model.complete)
        }
    }

    private var backup: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            notice(Localized.bsvHdBackupHint, warning: true)
            if let backup = model.backup {
                Text(backup).font(.body.weight(.medium)).privacySensitive().wowPayCard()
            }
        }
    }

    @ViewBuilder
    private var reviewDetails: some View {
        if let review = model.pending?.review {
            VStack(alignment: .leading, spacing: Spacing.space12) {
                value(Localized.Transfer.network, Localized.multisigMainnet)
                value(Localized.Wallet.name, model.wallet?.name ?? "")
                value(Localized.Common.address, review.recipient)
                value(Localized.wowpayAmount, BsvColdWalletAmount.display(review.amount) + " BSV")
                value(Localized.Transfer.networkFee, BsvColdWalletAmount.display(review.fee) + " BSV")
                value(Localized.multisigFundingTotal, BsvColdWalletAmount.display(review.totalDebit) + " BSV")
                value(Localized.multisigChangeOutput, BsvColdWalletAmount.display(review.change) + " BSV")
                if let address = review.changeAddress {
                    value(Localized.coldWalletChangeAddress, address)
                }
                value(Localized.coldWalletInputCount, String(review.inputCount))
                value(Localized.coldWalletRequestIdentity, review.commitment)
            }
            .wowPayCard()
        }
    }

    private var nameField: some View {
        @Bindable var model = model
        return TextField(Localized.Wallet.name, text: $model.walletName).wowPayInput().disabled(model.isBusy)
    }

    private func primary(_ title: String, enabled: Bool = true, action: @escaping () -> Void) -> some View {
        Button(title, action: action).buttonStyle(.wowPay()).disabled(!enabled || model.isBusy)
    }

    private func secondary(_ title: String, enabled: Bool = true, action: @escaping () -> Void) -> some View {
        Button(title, action: action).buttonStyle(.wowPaySecondary()).disabled(!enabled || model.isBusy)
    }

    private func value(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.space6) {
            Text(title).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
            Text(value).font(.body.weight(.medium)).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func notice(_ text: String, warning: Bool = false) -> some View {
        Text(text).font(.caption)
            .foregroundStyle(warning ? Colors.red : Colors.WowPay.secondaryText)
            .frame(maxWidth: .infinity, alignment: .leading).wowPayCard()
    }

    private func title(_ route: BsvColdWalletRoute) -> String {
        switch route {
        case .signing: Localized.coldWalletSigning
        case .create: Localized.coldWalletCreate
        case .recover: Localized.coldWalletRecover
        case .connect: Localized.coldWalletConnect
        case .importReview: Localized.bsvHdPreviewTitle
        case .pairing: Localized.coldWalletPair
        case .workspaceHome, .overview: Localized.coldWalletTitle
        case .receive: Localized.Wallet.receive
        case .send: Localized.Wallet.send
        case .request: Localized.coldWalletCreateRequest
        case .review: Localized.coldWalletReview
        case .signed: Localized.coldWalletSigned
        case .broadcast: Localized.multisigBroadcast
        case .result: Localized.coldWalletBroadcastSuccess
        case .backup: Localized.bsvHdBackupTitle
        }
    }

    private func copy(_ value: String) {
        guard !value.isEmpty else { return }
        UIPasteboard.general.setItems([[UIPasteboard.typeAutomatic: value]], options: [.localOnly: true])
    }

    private func firstAddress(_ wallet: BsvHomeCore.Wallet) -> String {
        wallet.accounts.first { $0.chain == Primitives.Chain.bitcoinSv.rawValue }?.address ?? ""
    }

    private func updateCapture() {
        let captured = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.contains { $0.traitCollection.sceneCaptureState == .active }
        model.captureChanged(captured)
    }
}
