import Components
import Foundation
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import enum BsvHomeCore.BsvHomeBsvHdImportMode
import protocol BsvHomeCore.BsvHomeKeystoreProtocol
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import enum BsvHomeCore.BsvHomeWalletImportKind
import enum BsvHomeCore.BsvHomeWalletImportResult
import enum BsvHomeCore.BsvHomeWalletImportType
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

@Observable
@MainActor
final class ImportWalletSceneViewModel {
    private let service: any BsvHomeWalletServiceProtocol
    private let keystore: any BsvHomeKeystoreProtocol
    private let authenticate: @MainActor @Sendable () async throws -> Void
    private let appStartService: any BsvHomeAppStartServiceProtocol
    private let preferences: ObservablePreferences
    private let wordSuggester = WordSuggester()
    private let onComplete: (@MainActor @Sendable (ImportWalletSceneResult) -> Void)?
    private let purpose: WalletOnboardingPurpose
    private var pendingIdentityImport: BsvHomeWalletImportResult?
    private var contextID = UUID()
    private var operationID: UUID?
    private var importTaskID: UUID?
    private(set) var importTask: Task<Void, Never>?
    private var scanFrames: [String] = []
    private var scanID = UUID()
    private var isProcessingScan = false
    private var isScreenAvailable = true
    let type: ImportWalletType
    let nameRecordViewModel: NameRecordViewModel?

    var input = "" {
        didSet {
            if input != oldValue {
                invalidatePreview()
            }
        }
    }

    var passphrase = "" {
        didSet {
            if passphrase != oldValue {
                invalidatePreview()
            }
        }
    }

    var hdConfiguration = BsvHdImportConfiguration() {
        didSet {
            if hdConfiguration != oldValue {
                invalidatePreview()
            }
        }
    }

    var importType: BsvHomeWalletImportKind = .phrase {
        didSet {
            if importType != oldValue {
                clearSensitiveState()
            }
        }
    }

    var wordsSuggestion: [String] = []
    var buttonState = ButtonState.normal
    var isPresentingScanner = false
    var isPresentingHdReview = false
    var isHdConfirmed = false
    var isSecretVisible = false
    var isPresentingAlertMessage: AlertMessage?
    var isPresentingExistingWalletName: String?
    private(set) var preview: BsvHdImportPreview?
    private(set) var isBusy = false

    init(
        service: any BsvHomeWalletServiceProtocol,
        keystore: any BsvHomeKeystoreProtocol,
        authenticate: @escaping @MainActor @Sendable () async throws -> Void,
        appStartService: any BsvHomeAppStartServiceProtocol,
        preferences: ObservablePreferences,
        nameService: any BsvHomeNameServiceProtocol,
        type: ImportWalletType,
        onComplete: (@MainActor @Sendable (ImportWalletSceneResult) -> Void)?,
        purpose: WalletOnboardingPurpose = .personal,
    ) {
        self.service = service
        self.keystore = keystore
        self.authenticate = authenticate
        self.appStartService = appStartService
        self.preferences = preferences
        self.type = type
        self.onComplete = onComplete
        self.purpose = purpose
        nameRecordViewModel = switch type {
        case .multicoin: nil
        case .chain: NameRecordViewModel(nameService: nameService)
        }
    }

    var title: String {
        switch type {
        case .multicoin: Localized.Wallet.multicoin
        case let .chain(chain): chain.networkName
        }
    }

    var pasteButtonTitle: String { Localized.Common.paste }
    var pasteButtonImage: Image { Images.System.paste }
    var qrButtonTitle: String { Localized.Wallet.scan }
    var qrButtonImage: Image { Images.System.qrCodeViewfinder }
    var alertTitle: String { Localized.Errors.validation("") }
    var chain: Chain? { type.chain }
    var showImportTypes: Bool { importTypes.count > 1 }
    var importTypes: [BsvHomeWalletImportKind] { service.importKinds(chain: chain?.map()) }
    var footerText: String? { importType.showsViewOnlyWarning() ? Localized.Wallet.importAddressWarning : nil }
    var docsUrl: URL { AppUrl.docs(.howToSecureSecretPhrase) }
    var shouldProtectInput: Bool { importType.protectsInput() }
    var showsHdSettings: Bool { !purpose.isDedicatedMultisigIdentity && importType != .privateKey && (chain == nil || chain == .bitcoinSv) }
    var hdImportMode: BsvHomeBsvHdImportMode? {
        service.bsvHdImportMode(kind: importType, chain: chain?.map(), input: input, hasOverrides: hdConfiguration.hasOverrides(passphrase: passphrase))
    }

    var requiresHdPreview: Bool { !purpose.isDedicatedMultisigIdentity && (hdImportMode == .recovery || hdImportMode == .observer) }
    var hasPendingIdentityImport: Bool { pendingIdentityImport != nil }
    var hasRecoveryBackupInput: Bool { input.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("{") || input.contains(":") }
    var showsWordSuggestions: Bool { importType == .phrase && !hasRecoveryBackupInput && !isBusy }
    var canSubmit: Bool { !isBusy && (hasPendingIdentityImport || (!input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && (!showsHdSettings || hdConfiguration.isComplete))) }
    var scanHint: String { scanFrames.isEmpty ? Localized.bsvHdPublicInput : Localized.coldWalletQrProgress(scanFrames.count) }
    var scanType: QRScanType {
        switch importType {
        case .phrase: .secretPhrase
        case .privateKey: .privateKey
        case .address: chain == .bitcoinSv ? .universal : .address
        }
    }
}

extension ImportWalletSceneViewModel {
    func startImport() {
        guard importTask == nil, isScreenAvailable else { return }
        let ticket = UUID()
        importTaskID = ticket
        importTask = Task {
            defer {
                if importTaskID == ticket {
                    importTask = nil
                    importTaskID = nil
                }
            }
            await onSelectActionButton()
        }
    }

    func onChangeImportType(_: BsvHomeWalletImportKind, _: BsvHomeWalletImportKind) {
        wordsSuggestion = []
        nameRecordViewModel?.reset()
    }

    func onChangeInput(_: String, newValue: String) {
        wordsSuggestion = showsWordSuggestions ? wordSuggester.wordSuggestionCalculate(value: newValue) : []
        if importType == .address, !requiresHdPreview, let chain {
            nameRecordViewModel?.getNameRecord(name: newValue, chain: chain)
        } else {
            nameRecordViewModel?.reset()
        }
    }

    func onSelectActionButton() async {
        guard canSubmit, isScreenAvailable, !Task.isCancelled else { return }
        let operation = beginOperation()
        let context = contextID
        do {
            if hasPendingIdentityImport {
                try completePendingIdentityImport(context: context, operation: operation)
            } else if requiresHdPreview {
                try await prepareHdPreview(context: context)
            } else {
                try await importWallet(context: context, operation: operation)
            }
        } catch is CancellationError {
            if operationID == operation, hasPendingIdentityImport {
                clearSensitiveInput()
            }
        } catch {
            if isCurrent(context: context, operation: operation) {
                if requiresHdPreview {
                    clearSensitiveInput()
                    isPresentingAlertMessage = AlertMessage(message: Localized.bsvHdInvalid)
                } else {
                    if hasPendingIdentityImport {
                        clearSensitiveInput()
                    }
                    isPresentingAlertMessage = AlertMessage(title: alertTitle, error: error)
                }
            }
        }
        endOperation(operation)
    }

    func confirmHdImport() async {
        guard !isBusy, isScreenAvailable, isHdConfirmed, let preview, preview.contextID == contextID else { return }
        let operation = beginOperation()
        var importedWallet: Wallet?
        do {
            try await authenticate()
            try checkContext(preview.contextID, operation: operation)
            guard self.preview?.id == preview.id, isHdConfirmed else { throw CancellationError() }
            let defaultName = try await service.defaultWalletName(chain: Chain.bitcoinSv.map()).name
            try checkContext(preview.contextID, operation: operation)
            let service = service
            let input = input
            let configuration = hdConfiguration
            let passphrase = passphrase
            let imported = try await Task.detached(priority: .userInitiated) {
                switch preview.mode {
                case .recovery:
                    return try await service.bsvHdImport(
                        name: defaultName,
                        recoveryString: input,
                        accountPath: configuration.accountPath,
                        passphrase: passphrase.isEmpty ? nil : passphrase,
                        expectedAddress: preview.account.address,
                        legacyNonCompliant: configuration.legacyNonCompliant,
                    )
                case .observer:
                    return try await service.bsvColdWalletImportPairing(name: defaultName, pairingJson: preview.pairing)
                case .multicoin:
                    throw CancellationError()
                }
            }.value
            try checkContext(preview.contextID, operation: operation)
            let result: WalletImportResult = switch imported {
            case let .new(wallet): .new(wallet.map())
            case let .existing(wallet): .existing(wallet.map())
            }
            let wallet = result.wallet
            importedWallet = wallet
            let actualPairing = try await Task.detached(priority: .userInitiated) { try await service.bsvHdPairing(walletId: wallet.id.id) }.value
            try checkContext(preview.contextID, operation: operation)
            guard actualPairing == preview.pairing else { throw CocoaError(.fileReadCorruptFile) }
            let account = try wallet.account(for: .bitcoinSv)
            guard account.address == preview.account.address,
                  account.derivationPath == preview.account.derivationPath
            else { throw CocoaError(.fileReadCorruptFile) }
            preferences.acceptTerms()
            try service.setCurrentWalletId(walletId: wallet.id.id)
            clearSensitiveInput()
            do {
                try await appStartService.initializeWalletAssets(wallet: wallet.map())
            } catch {
                guard operationID == operation else { return }
                isPresentingHdReview = false
                isPresentingAlertMessage = AlertMessage(message: Localized.coldWalletImportRefreshFailed, actions: [
                    AlertAction(title: Localized.Common.continue) { [weak self] in
                        Task { @MainActor in self?.completeImport(result) }
                    },
                ])
                endOperation(operation)
                return
            }
            guard operationID == operation else { return }
            isPresentingHdReview = false
            completeImport(result)
        } catch is CancellationError {
            if isCurrent(context: preview.contextID, operation: operation) {
                clearSensitiveInput()
            }
        } catch {
            if isCurrent(context: preview.contextID, operation: operation) {
                clearSensitiveInput()
                isPresentingAlertMessage = AlertMessage(message: importedWallet == nil ? Localized.bsvHdInvalid : Localized.coldWalletStorageProtectionFailed)
            }
        }
        endOperation(operation)
    }

    func onSelectScanQR() {
        guard !isBusy else { return }
        scanID = UUID()
        scanFrames = []
        isPresentingScanner = true
    }

    func onHandleScan(_ result: String) async {
        guard !isBusy, isPresentingScanner else { return }
        if importType != .address || chain != .bitcoinSv {
            input = result
            isPresentingScanner = false
            return
        }
        guard !isProcessingScan else { return }
        let session = scanID
        isProcessingScan = true
        defer { isProcessingScan = false }
        do {
            guard result.utf8.count <= 1_048_576 else { throw CocoaError(.fileReadTooLarge) }
            let header = try? JSONDecoder().decode(BsvHdQrHeader.self, from: Data(result.utf8))
            let payload: String
            if header?.kind == "wowpay-bsv-qr" {
                guard scanFrames.count < 1800 || scanFrames.contains(result) else { throw CocoaError(.fileReadTooLarge) }
                let candidate = scanFrames.contains(result) ? scanFrames : scanFrames + [result]
                let keystore = keystore
                let assembled = try await Task.detached(priority: .userInitiated) { try keystore.bsvColdQrAssemble(frames: candidate) }.value
                guard session == scanID, isPresentingScanner else { return }
                scanFrames = candidate
                guard let assembled else { return }
                payload = assembled
            } else {
                payload = result
            }
            let mode = service.bsvHdImportMode(kind: .address, chain: Chain.bitcoinSv.map(), input: payload, hasOverrides: hdConfiguration.hasOverrides(passphrase: ""))
            if mode == .observer {
                let configuration = hdConfiguration
                let keystore = keystore
                let validated = try await Task.detached(priority: .userInitiated) {
                    try keystore.bsvHdPublicPreview(input: payload, accountPath: configuration.accountPath, expectedAddress: configuration.originalAddress)
                }.value
                guard session == scanID, isPresentingScanner else { return }
                input = validated
            } else {
                input = payload
            }
            isPresentingScanner = false
            scanFrames = []
        } catch {
            guard session == scanID else { return }
            scanFrames = []
            isPresentingScanner = false
            isPresentingAlertMessage = AlertMessage(message: Localized.bsvHdInvalid)
        }
    }

    func onSelectWord(_ word: String) {
        guard !isBusy else { return }
        input = wordSuggester.selectWordCalculate(input: input, word: word)
    }

    func onPaste() {
        guard !isBusy, let string = UIPasteboard.general.string else { return }
        input = string
        if shouldProtectInput {
            CopyTypeViewModel.clearClipboard()
        }
    }

    func onSelectExistingWalletContinue() {
        onComplete?(.existing)
    }

    func setScreenAvailability(_ available: Bool) {
        isScreenAvailable = available
    }

    func clearSensitiveState() {
        importTask?.cancel()
        importTask = nil
        importTaskID = nil
        operationID = nil
        isBusy = false
        buttonState = .normal
        clearSensitiveInput()
        isPresentingHdReview = false
        isPresentingScanner = false
        scanFrames = []
        scanID = UUID()
        nameRecordViewModel?.reset()
    }

    private func completeImport(_ result: WalletImportResult) {
        switch result {
        case let .new(wallet): onComplete?(.new(wallet))
        case let .existing(wallet): isPresentingExistingWalletName = wallet.name
        }
    }

    private func clearSensitiveInput() {
        input = ""
        passphrase = ""
        wordsSuggestion = []
        isSecretVisible = false
        preview = nil
        isHdConfirmed = false
    }

    private func invalidatePreview() {
        contextID = UUID()
        preview = nil
        isHdConfirmed = false
        if !isBusy {
            isPresentingHdReview = false
        }
    }

    private func beginOperation() -> UUID {
        let id = UUID()
        operationID = id
        isBusy = true
        buttonState = .loading(showProgress: true)
        return id
    }

    private func endOperation(_ operation: UUID) {
        guard operationID == operation else { return }
        operationID = nil
        isBusy = false
        buttonState = .normal
        if preview == nil {
            isPresentingHdReview = false
        }
    }

    private func isCurrent(context: UUID, operation: UUID) -> Bool {
        contextID == context && operationID == operation
    }

    private func checkContext(_ context: UUID, operation: UUID) throws {
        try Task.checkCancellation()
        guard isScreenAvailable, isCurrent(context: context, operation: operation) else { throw CancellationError() }
    }

    private func prepareHdPreview(context: UUID) async throws {
        let input = input
        let configuration = hdConfiguration
        let passphrase = passphrase
        let mode = hdImportMode
        let keystore = keystore
        let pairing = try await Task.detached(priority: .userInitiated) {
            if mode == .observer {
                return try keystore.bsvHdPublicPreview(input: input, accountPath: configuration.accountPath, expectedAddress: configuration.originalAddress)
            }
            return try keystore.bsvHdPreview(
                recoveryString: input,
                accountPath: configuration.accountPath,
                passphrase: passphrase.isEmpty ? nil : passphrase,
                expectedAddress: configuration.originalAddress,
                legacyNonCompliant: configuration.legacyNonCompliant,
            )
        }.value
        guard context == contextID, let mode, mode != .multicoin else { throw CancellationError() }
        let accountJson = try await Task.detached(priority: .userInitiated) { try keystore.bsvColdPairingValidate(descriptorJson: pairing) }.value
        guard context == contextID else { throw CancellationError() }
        let account = try JSONDecoder().decode(Account.self, from: Data(accountJson.utf8))
        preview = BsvHdImportPreview(id: UUID(), contextID: context, mode: mode, pairing: pairing, account: account)
        isHdConfirmed = false
        isPresentingHdReview = true
    }

    private func importWallet(context: UUID, operation: UUID) async throws {
        let nameRecord = nameRecordViewModel?.state.record()
        let defaultName = try await service.defaultWalletName(chain: chain?.map()).name
        try checkContext(context, operation: operation)
        let request = try service.importRequest(kind: importType, chain: chain?.map(), input: input, nameRecord: nameRecord)
        let result = try await service.importWallet(name: service.importName(nameRecord: nameRecord, defaultName: defaultName), import: request, source: .import)
        if purpose.isDedicatedMultisigIdentity {
            pendingIdentityImport = result
            try checkContext(context, operation: operation)
            try completePendingIdentityImport(context: context, operation: operation)
            return
        }
        try checkContext(context, operation: operation)
        let wallet: Wallet = switch result {
        case let .new(wallet), let .existing(wallet): wallet.map()
        }
        try await appStartService.initializeWalletAssets(wallet: wallet.map())
        try checkContext(context, operation: operation)
        preferences.acceptTerms()
        try service.setCurrentWalletId(walletId: wallet.id.id)
        switch result {
        case .new: clearSensitiveInput(); onComplete?(.new(wallet))
        case .existing: isPresentingExistingWalletName = wallet.name
        }
    }

    private func completePendingIdentityImport(context: UUID, operation: UUID) throws {
        guard let result = pendingIdentityImport, case let .dedicatedMultisigIdentity(onComplete) = purpose else { return }
        try checkContext(context, operation: operation)
        let wallet = try service.completeMultisigIdentityImport(result: result, source: .import)
        try checkContext(context, operation: operation)
        pendingIdentityImport = nil
        preferences.acceptTerms()
        clearSensitiveInput()
        onComplete(wallet.map())
    }
}
