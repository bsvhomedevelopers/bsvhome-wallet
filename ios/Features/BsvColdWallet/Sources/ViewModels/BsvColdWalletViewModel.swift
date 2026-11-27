import BigInt
import Components
import Foundation
import BsvHomeCore
import Localization
import PrimitivesComponents
import SwiftUI

@Observable
@MainActor
public final class BsvColdWalletViewModel {
    private let service: any BsvHomeBsvColdWalletServiceProtocol
    private let preferredWalletId: String?
    private let entry: BsvColdWalletEntry
    private let cancelAuthorization: @MainActor (String) -> Void
    private let onComplete: @MainActor () -> Void
    private let protectStorage: @Sendable () async throws -> Void
    public let mode: BsvColdWalletMode
    @ObservationIgnored private(set) var operationTask: Task<Void, Never>?
    @ObservationIgnored private var pageNavigation: BsvColdWalletNavigation?
    @ObservationIgnored private var actionNavigation: (scopeId: String, owner: NavigationPathState, observerId: UUID)?
    private var contextId = UUID()
    private var scanFrames: [String] = []
    private var previewPairing: String?
    private var previewId: UUID?
    private var retireNavigation: (@MainActor () -> Void)?
    private var activeScope: BsvHomeBsvColdWalletActionScope?
    private var recipientScannerId: UUID?
    private var importIsSecret = false
    public private(set) var isCancelled = false
    private(set) var actionOutcome: BsvHomeBsvColdWalletActionOutcome?
    private var requiresStateReload = false
    private var hasLoaded = false
    private enum InitialRequestScan { case idle, pending, consumed }
    private var initialRequestScan = InitialRequestScan.idle
    private var needsReload = false
    @ObservationIgnored private var interruptedPublicPath: [BsvColdWalletRoute]?

    var path: [BsvColdWalletRoute] = []
    var wallets: [BsvHomeCore.Wallet] = []
    var candidates: [BsvHomeBsvColdWalletCandidate] = []
    var snapshot: BsvHomeBsvColdWalletSnapshot?
    var pending: BsvHomeBsvColdWalletPending?
    var addresses: BsvHomeBsvColdAddressState?
    var secretKind: BsvHomeWalletSecretKind?
    var isBusy = false
    var isForeground = true
    var isCaptured = false
    var errorMessage: String?
    private(set) var loadFailed = false
    var backupError: String?
    var storageProtectionFailed = false
    private(set) var scannerSession: BsvColdWalletScanner?
    var scanner: BsvColdWalletScan? { scannerSession?.kind }
    var replacementRequest: String?
    var receivedFrames = 0
    var frames: [String] = []
    var words: [String] = []
    var backup: String?
    var walletName = "" { didSet {
        if oldValue != walletName {
            invalidatePreview()
        }
    } }
    var input = "" { didSet {
        if oldValue != input {
            invalidatePreview()
        }
    } }
    var passphrase = "" { didSet {
        if oldValue != passphrase {
            invalidatePreview()
        }
    } }
    var configuration = BsvHdImportConfiguration() { didSet {
        if oldValue != configuration {
            invalidatePreview()
        }
    } }
    var previewPath = ""
    var previewAddress = ""
    var isPreviewConfirmed = false
    var isBackupConfirmed = false
    var isReviewConfirmed = false
    var recipient = ""
    var amount = ""

    public init(service: any BsvHomeBsvColdWalletServiceProtocol, mode: BsvColdWalletMode, walletId: String? = nil, initialPage: BsvColdWalletEntry = .home, protectStorage: @escaping @Sendable () async throws -> Void = {}, cancelAuthorization: @escaping @MainActor (String) -> Void = { _ in }, onComplete: @escaping @MainActor () -> Void = {}) {
        self.service = service
        self.mode = mode
        preferredWalletId = walletId
        entry = initialPage
        self.protectStorage = protectStorage
        self.cancelAuthorization = cancelAuthorization
        self.onComplete = onComplete
        importIsSecret = mode == .offline || initialPage == .recover || initialPage == .create
    }

    var entryIsPairing: Bool { entry == .pairing }
    var initialRoute: BsvColdWalletRoute? {
        switch entry {
        case .home: nil
        case .scanRequest: .workspaceHome
        case .create: .create
        case .recover: .recover
        case .connect: .connect
        case .signing: .signing
        case .pairing: .pairing
        case .send: .send
        case .receive: .receive
        }
    }

    private var signingWorkspacePath: [BsvColdWalletRoute] {
        if case .scanRequest = entry {
            return [.workspaceHome, .signing]
        }
        return [.signing]
    }

    private func signingPath(to page: BsvColdWalletRoute) -> [BsvColdWalletRoute] {
        guard mode == .unified else { return [page] }
        switch entry {
        case .scanRequest, .signing: return signingWorkspacePath + [page]
        case .home, .create, .recover, .connect, .pairing, .send, .receive: return [page]
        }
    }

    var wallet: BsvHomeCore.Wallet? { snapshot?.wallet }
    var isOffline: Bool { mode == .offline }
    var canHoldSecrets: Bool { mode != .online }
    var isSecretImport: Bool { importIsSecret }
    var isObserver: Bool { snapshot?.role == .observer }
    var isSigner: Bool { snapshot?.role == .signer }
    var hasSubmittedBroadcast: Bool { pending?.broadcasted == true || actionOutcome?.submission == .submitted }
    var transactionId: String { actionOutcome?.transactionId ?? pending?.transactionId ?? "" }
    var hasPersistenceWarning: Bool { pending?.persistenceError != nil || actionOutcome?.requiresReload == true || requiresStateReload }
    var canPrepare: Bool { !isCancelled && !isOffline && isObserver && pending?.broadcasted != false && !isBusy && !requiresStateReload }
    var canSign: Bool { !isCancelled && canHoldSecrets && isSigner && pending != nil && isReviewConfirmed && !isBusy && !isCaptured && !storageProtectionFailed && !requiresStateReload }
    var canBroadcast: Bool { !isCancelled && !isOffline && isObserver && pending?.signedJson != nil && !hasSubmittedBroadcast && !isBusy && !isCaptured && !requiresStateReload }
    var canImport: Bool { !isCancelled && !walletName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && walletName.utf16.count <= 30 && !input.isEmpty && input.utf8.count <= 8192 && passphrase.count <= 1024 && configuration.isComplete && !isBusy && !storageProtectionFailed }
    var selectionCandidates: [BsvHomeBsvColdWalletCandidate] {
        let selectingSigner = path.last == .signing || mode == .offline
        return candidates.filter { $0.role == (selectingSigner ? .signer : .observer) }
    }

    var balance: String {
        guard let available: BigUInt = snapshot?.balance?.available, let value = UInt64(exactly: available) else { return "—" }
        return BsvColdWalletAmount.display(value) + " BSV"
    }

    var receiveAddress: String { addresses?.addresses.first(where: { $0.index == addresses?.selectedIndex })?.address ?? "" }

    func navigation(in owner: NavigationPathState) -> BsvColdWalletNavigation {
        if let pageNavigation {
            return pageNavigation
        }
        let navigation = BsvColdWalletNavigation(navigation: owner)
        pageNavigation = navigation
        return navigation
    }

    func load() async {
        await startLoading()?.value
    }

    func retryLoad() {
        guard loadFailed else { return }
        startLoading()
    }

    @discardableResult
    private func startLoading() -> Task<Void, Never>? {
        guard !isCancelled, !hasLoaded || needsReload || loadFailed else { return nil }
        return operation { context, generation in
            do {
                let candidates = try await self.service.walletCandidates()
                try self.checkContext(context)
                let initial = !self.hasLoaded
                let currentIdentifier = try self.service.currentWalletId()
                let identifier = initial ? self.preferredWalletId ?? currentIdentifier : currentIdentifier
                self.candidates = candidates
                self.wallets = candidates.map(\.wallet)
                if self.canHoldSecrets {
                    do {
                        try await self.protectStorage()
                        try self.checkContext(context)
                        self.storageProtectionFailed = false
                    } catch {
                        try self.checkContext(context)
                        self.storageProtectionFailed = true
                        self.errorMessage = Localized.coldWalletStorageProtectionFailed
                    }
                }
                if initial, self.entry == .pairing || self.entry == .signing, let identifier = self.preferredWalletId {
                    guard self.canHoldSecrets, !self.storageProtectionFailed else { throw BsvHomeServiceError.Cancelled }
                    try await self.exportPairingSnapshot(walletId: identifier, context: context, generation: generation)
                } else if let identifier, candidates.contains(where: { $0.wallet.id == identifier }) {
                    if initial, case .scanRequest = self.entry {
                        try await self.service.selectWallet(walletId: identifier)
                        try self.checkContext(context)
                    }
                    try await self.loadWallet(identifier, context: context)
                } else {
                    self.snapshot = nil
                    self.pending = nil
                    self.addresses = nil
                    self.secretKind = nil
                }
                self.hasLoaded = true
                self.requiresStateReload = false
                if !self.storageProtectionFailed, self.path.isEmpty, let publicPath = self.interruptedPublicPath {
                    self.interruptedPublicPath = nil
                    self.initialRequestScan = .consumed
                    self.isReviewConfirmed = false
                    self.path = publicPath
                } else if initial, !self.needsReload, !self.storageProtectionFailed {
                    try await self.applyEntry(context: context, generation: generation)
                }
                self.needsReload = false
            } catch {
                try self.checkContext(context)
                if error is CancellationError {
                    throw error
                }
                if let error = error as? BsvHomeServiceError, case .Cancelled = error {
                    throw error
                }
                self.loadFailed = true
                throw error
            }
        }
    }

    func selectWallet(_ identifier: String) {
        let selectable = mode == .unified ? selectionCandidates : candidates
        guard selectable.contains(where: { $0.wallet.id == identifier }) else { return }
        initialRequestScan = .consumed
        let wasSigningWorkspace = mode == .unified && path.contains(.signing)
        operation { context, _ in
            try await self.service.selectWallet(walletId: identifier)
            try self.checkContext(context)
            self.clearSecrets()
            self.scanFrames = []
            self.pending = nil
            self.snapshot = nil
            self.addresses = nil
            self.secretKind = nil
            self.frames = []
            self.recipient = ""
            self.amount = ""
            self.actionOutcome = nil
            self.path = self.mode == .unified ? (wasSigningWorkspace ? self.signingWorkspacePath : []) : (self.entry == .signing ? [.signing] : [])
            try await self.loadWallet(identifier, context: context)
            switch self.entry {
            case .pairing:
                if self.isSigner {
                    try await self.pairingFrames(context: context); self.path = [.pairing]
                }
            case .send: if self.isObserver {
                    self.path = [.send]
                }
            case .receive: if self.isObserver {
                    self.path = [.receive]
                }
            case .home, .create, .recover, .connect, .signing, .scanRequest: break
            }
        }
    }

    func open(_ route: BsvColdWalletRoute) {
        guard !isCancelled, !isBusy else { return }
        errorMessage = nil
        if route == .recover || route == .connect {
            clearSecrets()
            walletName = ""
            importIsSecret = route == .recover
        }
        path.append(route)
    }

    func generateWords() {
        operation { context, generation in
            guard self.canHoldSecrets else { throw BsvHomeServiceError.Cancelled }
            let words = try await self.local { try $0.createWords(expectedGeneration: generation) }
            try self.checkContext(context)
            self.clearSecrets()
            self.words = words
            self.walletName = ""
            self.path.append(.create)
        }
    }

    func create() {
        guard isBackupConfirmed, walletName.trimmingCharacters(in: .whitespacesAndNewlines).count > 0, walletName.utf16.count <= 30 else { return }
        let name = walletName.trimmingCharacters(in: .whitespacesAndNewlines)
        let words = words
        operation { context, generation in
            let snapshot = try await self.service.create(name: name, words: words, expectedGeneration: generation)
            try self.checkContext(context)
            self.clearSecrets()
            try await self.acceptImport(snapshot, context: context)
        }
    }

    func previewImport() {
        guard canImport else { return }
        operation { context, generation in
            let input = self.input
            let path = self.configuration.accountPath
            let passphrase = self.passphrase.isEmpty ? nil : self.passphrase
            let address = self.configuration.originalAddress
            let legacy: Bool? = self.configuration.legacyNonCompliant ? true : nil
            let pairing: String = if self.isSecretImport {
                try await self.local { try $0.previewRecovery(input: input, accountPath: path, passphrase: passphrase, expectedAddress: address, legacyNonCompliant: legacy, expectedGeneration: generation) }
            } else {
                try await self.local { try $0.previewObserver(input: input, accountPath: path, expectedAddress: address) }
            }
            try self.checkContext(context)
            try self.applyPreview(pairing)
        }
    }

    func confirmImport() {
        guard isPreviewConfirmed, let pairing = previewPairing, let expectedId = previewId else { return }
        operation { context, generation in
            let snapshot: BsvHomeBsvColdWalletSnapshot
            do {
                guard self.previewId == expectedId else { throw BsvHomeServiceError.Cancelled }
                if self.isSecretImport {
                    snapshot = try await self.service.importRecovery(name: self.walletName.trimmingCharacters(in: .whitespacesAndNewlines), input: self.input, accountPath: self.configuration.accountPath, passphrase: self.passphrase.isEmpty ? nil : self.passphrase, pairingJson: pairing, legacyNonCompliant: self.configuration.legacyNonCompliant ? true : nil, expectedGeneration: generation)
                } else {
                    snapshot = try await self.service.importObserver(name: self.walletName.trimmingCharacters(in: .whitespacesAndNewlines), pairingJson: pairing, expectedGeneration: generation)
                }
            } catch {
                try self.checkContext(context)
                if let serviceError = error as? BsvHomeServiceError, case .Cancelled = serviceError {
                    throw error
                }
                self.clearSecrets()
                self.path = [self.isSecretImport ? .recover : .connect]
                throw error
            }
            try self.checkContext(context)
            guard self.previewId == expectedId else { throw BsvHomeServiceError.Cancelled }
            self.clearSecrets()
            try await self.acceptImport(snapshot, context: context)
        }
    }

    func exportPairing() {
        guard let wallet, canHoldSecrets, !storageProtectionFailed else { return }
        operation { context, generation in
            try await self.exportPairingSnapshot(walletId: wallet.id, context: context, generation: generation)
            try await self.pairingFrames(context: context)
            self.path.append(.pairing)
        }
    }

    func exportSecret() {
        guard let wallet, isSigner, secretKind != nil, !storageProtectionFailed else { return }
        operation { context, generation in
            let secret = try await self.service.exportSecret(walletId: wallet.id, expectedGeneration: generation)
            try self.checkContext(context)
            switch secret {
            case let .words(words): self.backup = words.joined(separator: " ")
            case let .recovery(recovery): self.backup = recovery
            case .privateKey: throw BsvHomeServiceError.Cancelled
            }
            self.path.append(.backup)
        }
    }

    func refresh() {
        guard let wallet, !isOffline, isObserver else { return }
        operation { context, _ in
            let snapshot = try await self.service.refresh(walletId: wallet.id)
            try self.checkContext(context)
            self.apply(snapshot)
        }
    }

    func retryStorageProtection() {
        operation { context, _ in
            try await self.protectStorage()
            try self.checkContext(context)
            if let wallet = self.wallet {
                try await self.loadWallet(wallet.id, context: context)
            }
            self.storageProtectionFailed = false
        }
    }

    func addAddress() {
        guard let wallet, addresses?.canAdd == true else { return }
        operation { context, _ in
            let addresses = try await self.service.addAddress(walletId: wallet.id)
            try self.checkContext(context)
            self.addresses = addresses
        }
    }

    func selectAddress(_ index: UInt32) {
        guard let wallet else { return }
        operation { context, _ in
            let addresses = try await self.service.selectAddress(walletId: wallet.id, index: index)
            try self.checkContext(context)
            self.addresses = addresses
        }
    }

    func prepare(navigation: NavigationPathState) {
        guard canPrepare, let wallet, let value = BsvColdWalletAmount.satoshis(amount, decimalSeparator: Locale.current.decimalSeparator ?? ".") else { return }
        let recipient = recipient.trimmingCharacters(in: .whitespacesAndNewlines)
        financial(walletId: wallet.id, intent: .prepare(recipient: recipient, amount: value), navigation: navigation) { context, scope in
            let pending = try await self.service.prepare(walletId: wallet.id, recipient: recipient, amount: value, scope: scope)
            try self.checkContext(context)
            let frames = try await self.local { try $0.qrFrames(payloadJson: pending.requestJson) }
            try self.checkContext(context)
            self.pending = pending
            self.frames = frames
            self.path.append(.request)
        }
    }

    func sign(navigation: NavigationPathState) {
        guard canSign, let wallet, let pending else { return }
        financial(walletId: wallet.id, intent: .sign(commitment: pending.review.commitment), navigation: navigation) { context, scope in
            let signed = try await self.service.sign(walletId: wallet.id, commitment: pending.review.commitment, scope: scope)
            try self.checkContext(context)
            self.pending = signed
            guard let payload = signed.signedJson else { throw BsvHomeServiceError.Store(msg: "Signed cold transaction is unavailable") }
            let frames = try await self.local { try $0.qrFrames(payloadJson: payload) }
            try self.checkContext(context)
            self.frames = frames
            self.isReviewConfirmed = false
            self.path = self.signingPath(to: .signed)
        }
    }

    func broadcast(navigation: NavigationPathState) {
        guard canBroadcast, let wallet, let pending else { return }
        financial(walletId: wallet.id, intent: .broadcast(commitment: pending.review.commitment), navigation: navigation) { context, scope in
            let result = try await self.service.broadcast(walletId: wallet.id, commitment: pending.review.commitment, scope: scope)
            try self.checkContext(context)
            self.pending = result
            self.actionOutcome = try scope.outcome()
            self.path = [.result]
            do {
                let snapshot = try await self.service.refresh(walletId: wallet.id)
                try self.checkContext(context)
                self.apply(snapshot)
            } catch {
                try self.checkContext(context)
                self.errorMessage = Localized.coldWalletRefreshFailed
            }
        }
    }

    func cancelRequest() {
        guard let wallet, let pending, pending.signedJson == nil else { return }
        operation { context, _ in
            try await self.service.cancelRequest(walletId: wallet.id, commitment: pending.review.commitment)
            try self.checkContext(context)
            self.frames = []
            if self.isSigner {
                let snapshot = try await self.service.snapshot(walletId: wallet.id)
                try self.checkContext(context)
                self.apply(snapshot)
                self.path = []
            } else {
                self.pending = nil
                self.path = [.send]
            }
        }
    }

    func showPending() {
        guard let pending else { return }
        operation { context, _ in
            let route: BsvColdWalletRoute = self.hasSubmittedBroadcast ? .result : self.isSigner ? pending.signedJson == nil ? .review : .signed : pending.signedJson == nil ? .request : .broadcast
            let payload = route == .request ? pending.requestJson : route == .signed ? pending.signedJson : nil
            let frames: [String] = if let payload {
                try await self.local { try $0.qrFrames(payloadJson: payload) }
            } else {
                []
            }
            try self.checkContext(context)
            self.frames = frames
            self.path = self.isSigner && (route == .review || route == .signed) ? self.signingPath(to: route) : [route]
        }
    }

    func activateInitialRequestScanner() {
        guard initialRequestScan == .pending, path.last == .signing, !isCancelled, isForeground, !isCaptured, isSigner, !storageProtectionFailed else { return }
        initialRequestScan = .consumed
        presentScanner(.request, origin: .initialEntry)
    }

    func requestSigning(using callback: (@MainActor (String) -> Void)?) {
        guard !isCancelled, !isBusy, isForeground, !isCaptured, isSigner, let wallet else { return }
        if let callback {
            callback(wallet.id)
        } else {
            startScan(.request)
        }
    }

    func requestPairing(using callback: @MainActor (String) -> Void) {
        guard !isCancelled, !isBusy, isForeground, !isCaptured, isSigner, canHoldSecrets, !storageProtectionFailed, let wallet else { return }
        callback(wallet.id)
    }

    func startScan(_ kind: BsvColdWalletScan) {
        guard !isCancelled, !isBusy else { return }
        presentScanner(kind, origin: .workspace)
    }

    private func presentScanner(_ kind: BsvColdWalletScan, origin: BsvColdWalletScanner.Origin) {
        guard !isCancelled, isForeground, !isCaptured else { return }
        switch kind {
        case .request: guard isSigner else { return }
        case .signed: guard isObserver else { return }
        case .pairing: guard !isOffline else { return }
        }
        scanFrames = []
        receivedFrames = 0
        errorMessage = nil
        scannerSession = BsvColdWalletScanner(id: UUID(), kind: kind, origin: origin)
    }

    @discardableResult
    func closeScanner(_ id: UUID) -> BsvColdWalletScanner.Origin? {
        guard let session = scannerSession, session.id == id else { return nil }
        let interrupted = operationTask != nil
        contextId = UUID()
        do { try service.invalidateActions() } catch { errorMessage = Localized.coldWalletStorageError }
        operationTask?.cancel()
        operationTask = nil
        isBusy = false
        scannerSession = nil
        scanFrames = []
        receivedFrames = 0
        if interrupted {
            requiresStateReload = true; needsReload = true; loadFailed = true
        }
        return session.origin
    }

    func scan(_ frame: String, scannerId: UUID) {
        guard scannerSession?.id == scannerId, let kind = scanner, !isCancelled, !scanFrames.contains(frame) else { return }
        guard frame.utf8.count <= 1_048_576 else {
            scanFrames = []
            receivedFrames = 0
            errorMessage = Localized.coldWalletInvalidData
            return
        }
        operation { context, _ in
            do {
                let payload: String?
                if kind == .pairing, frame.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("xpub") {
                    payload = frame
                } else {
                    guard let data = frame.data(using: .utf8), let json = try JSONSerialization.jsonObject(with: data) as? [String: Any], let businessKind = json["kind"] as? String else { throw BsvHomeServiceError.Cancelled }
                    if businessKind == "wowpay-bsv-qr" {
                        let candidate = self.scanFrames + [frame]
                        payload = try await self.local { try $0.qrAssemble(frames: candidate) }
                        try self.checkContext(context)
                        guard self.scannerSession?.id == scannerId else { return }
                        self.scanFrames = candidate
                        self.receivedFrames = candidate.count
                    } else {
                        guard businessKind == kind.kind else { throw BsvHomeServiceError.Cancelled }
                        payload = frame
                    }
                }
                guard let payload else { return }
                guard self.scannerSession?.id == scannerId, self.scanner == kind else { throw BsvHomeServiceError.Cancelled }
                switch kind {
                case .pairing:
                    let path = self.configuration.accountPath
                    let address = self.configuration.originalAddress
                    let pairing = try await self.local { try $0.previewObserver(input: payload, accountPath: path, expectedAddress: address) }
                    try self.checkContext(context)
                    guard self.scannerSession?.id == scannerId else { return }
                    self.input = payload
                    try self.applyPreview(pairing)
                case .request:
                    guard let wallet = self.wallet else { throw BsvHomeServiceError.Cancelled }
                    if self.pending?.signedJson != nil {
                        try self.checkContext(context)
                        self.replacementRequest = payload
                        self.scannerSession = nil
                        self.scanFrames = []
                        self.receivedFrames = 0
                        return
                    }
                    let pending = try await self.service.importRequest(walletId: wallet.id, requestJson: payload)
                    try self.checkContext(context)
                    self.pending = pending
                    self.isReviewConfirmed = false
                    self.scannerSession = nil
                    self.path = self.signingPath(to: .review)
                case .signed:
                    guard let wallet = self.wallet else { throw BsvHomeServiceError.Cancelled }
                    let pending = try await self.service.importSigned(walletId: wallet.id, signedJson: payload)
                    try self.checkContext(context)
                    self.pending = pending
                    self.path = [pending.broadcasted ? .result : .broadcast]
                }
                self.scannerSession = nil
                self.scanFrames = []
                self.receivedFrames = 0
            } catch {
                try self.checkContext(context)
                guard self.scannerSession?.id == scannerId else { return }
                self.scanFrames = []
                self.receivedFrames = 0
                self.errorMessage = Localized.coldWalletInvalidData
            }
        }
    }

    func sceneChanged(_ phase: ScenePhase) {
        isForeground = phase == .active
        if phase == .background {
            needsReload = true; invalidateSession()
        } else if phase == .active, needsReload || !hasLoaded || loadFailed {
            startLoading()
        }
    }

    func confirmReplacement(_ request: String) {
        guard let wallet, let old = pending else { return }
        replacementRequest = nil
        operation { context, _ in
            let pending = try await self.service.reviewRequest(walletId: wallet.id, requestJson: request, replaceCommitment: old.review.commitment)
            try self.checkContext(context)
            self.pending = pending
            self.isReviewConfirmed = false
            self.path = self.signingPath(to: .review)
        }
    }

    func captureChanged(_ captured: Bool) {
        let wasCaptured = isCaptured
        isCaptured = captured
        if captured {
            needsReload = true; invalidateSession()
        } else if wasCaptured, isForeground {
            startLoading()
        }
    }

    func navigationChanged(_ old: [BsvColdWalletRoute], _ new: [BsvColdWalletRoute]) {
        if old.last == .backup, new.last != .backup {
            backup = nil
        }
        if new.count < old.count {
            recipientScannerId = nil
            clearSecrets()
            isReviewConfirmed = false
        }
    }

    func navigationPopped(to depth: Int) {
        guard depth < path.count else { return }
        invalidateExecution(clearPath: false)
        path.removeSubrange(max(0, depth)...)
    }

    public func cancelActions() {
        needsReload = true
        invalidateExecution(clearPath: true)
    }

    public func cancel() {
        guard !isCancelled else { return }
        isCancelled = true
        cancelActions()
        retireNavigation?()
        retireNavigation = nil
    }

    public func cancelForNavigationRemoval() {
        retireNavigation = nil
        cancel()
    }

    func attachNavigationRetirement(_ retirement: @escaping @MainActor () -> Void) {
        guard !isCancelled else { return }
        retireNavigation = retirement
    }

    public func consumeWalletSelection(walletId: String) throws -> Bool {
        try service.consumeWalletSelection(walletId: walletId)
    }

    public func complete() {
        guard !isCancelled else { return }
        cancelActions()
        onComplete()
    }

    func disappear() {
        cancel()
    }

    private func invalidateSession() {
        cancelActions()
    }

    private func invalidateExecution(clearPath: Bool) {
        if clearPath {
            rememberInterruptedPublicPath()
        }
        initialRequestScan = .consumed
        contextId = UUID()
        let interrupted = operationTask != nil || activeScope != nil
        let scope = activeScope
        if let scope {
            removeActionNavigation(scopeId: scope.id())
        }
        activeScope = nil
        scope?.cancel()
        if let scope {
            cancelAuthorization(scope.id())
        }
        do { try service.invalidateActions() } catch { errorMessage = Localized.coldWalletStorageError }
        operationTask?.cancel()
        operationTask = nil
        isBusy = false
        if interrupted {
            requiresStateReload = true; loadFailed = true
        }
        scannerSession = nil
        recipientScannerId = nil
        replacementRequest = nil
        scanFrames = []
        receivedFrames = 0
        frames = []
        clearSecrets()
        if clearPath {
            path = []
        }
    }

    private func rememberInterruptedPublicPath() {
        guard mode == .unified else { return }
        let workspace: [BsvColdWalletRoute]
        switch entry {
        case .scanRequest, .signing: workspace = signingWorkspacePath
        case .home, .create, .recover, .connect, .pairing, .send, .receive:
            interruptedPublicPath = nil
            return
        }
        if workspace.first == .workspaceHome, path == [.workspaceHome] {
            interruptedPublicPath = [.workspaceHome]
        } else if path == workspace || path == workspace + [.review] || path == workspace + [.signed] {
            interruptedPublicPath = workspace
        } else if path.isEmpty {
            if interruptedPublicPath == nil, !hasLoaded, case .scanRequest(presentation: .resumed) = entry {
                interruptedPublicPath = workspace
            }
        } else {
            interruptedPublicPath = nil
        }
    }

    private func clearSecrets() {
        words = []
        input = ""
        passphrase = ""
        backup = nil
        previewPairing = nil
        previewId = nil
        previewPath = ""
        previewAddress = ""
        configuration = BsvHdImportConfiguration()
        isPreviewConfirmed = false
        isBackupConfirmed = false
        isReviewConfirmed = false
    }

    private func invalidatePreview() {
        previewPairing = nil
        previewId = nil
        isPreviewConfirmed = false
    }

    private func applyPreview(_ pairing: String) throws {
        let json = try JSONSerialization.jsonObject(with: Data(pairing.utf8)) as? [String: Any]
        guard let path = json?["accountPath"] as? String, let address = json?["firstAddress"] as? String else { throw BsvHomeServiceError.Cancelled }
        previewPairing = pairing
        previewId = UUID()
        previewPath = path
        previewAddress = address
        isPreviewConfirmed = false
        self.path.append(.importReview)
    }

    private func acceptImport(_ snapshot: BsvHomeBsvColdWalletSnapshot, context: UUID) async throws {
        apply(snapshot)
        if !wallets.contains(where: { $0.id == snapshot.wallet.id }) {
            wallets.append(snapshot.wallet)
            candidates.append(BsvHomeBsvColdWalletCandidate(wallet: snapshot.wallet, role: snapshot.role))
        }
        do {
            try await protectStorage()
            try checkContext(context)
        } catch {
            try checkContext(context)
            storageProtectionFailed = true
            errorMessage = Localized.coldWalletStorageProtectionFailed
            path = []
            return
        }
        candidates = try await service.walletCandidates()
        try checkContext(context)
        wallets = candidates.map(\.wallet)
        try await loadSecretKind(snapshot.wallet.id, context: context)
        if isSigner {
            frames = try await local { try $0.qrFrames(payloadJson: snapshot.pairingJson) }
            try checkContext(context)
            path = [.pairing]
        } else {
            path = [.overview]
            do {
                let refreshed = try await service.refresh(walletId: snapshot.wallet.id)
                try checkContext(context)
                apply(refreshed)
            } catch {
                try checkContext(context)
                errorMessage = Localized.coldWalletImportRefreshFailed
            }
        }
    }

    private func loadWallet(_ identifier: String, context: UUID) async throws {
        let snapshot = try await service.snapshot(walletId: identifier)
        try checkContext(context)
        apply(snapshot)
        try await loadSecretKind(identifier, context: context)
    }

    private func loadSecretKind(_ identifier: String, context: UUID) async throws {
        do {
            let kind = try await service.secretKind(walletId: identifier)
            try checkContext(context)
            secretKind = kind
            backupError = nil
        } catch {
            try checkContext(context)
            secretKind = nil
            backupError = Localized.bsvHdBackupUnavailable
        }
    }

    private func apply(_ snapshot: BsvHomeBsvColdWalletSnapshot) {
        self.snapshot = snapshot
        pending = snapshot.pending
        if snapshot.pending?.broadcasted == true {
            requiresStateReload = false
        }
        addresses = snapshot.addresses
    }

    private func checkContext(_ context: UUID) throws {
        try Task.checkCancellation()
        guard !isCancelled, context == contextId, isForeground, !isCaptured else { throw BsvHomeServiceError.Cancelled }
    }

    private func local<Value: Sendable>(_ operation: @escaping @Sendable (any BsvHomeBsvColdWalletServiceProtocol) throws -> Value) async throws -> Value {
        let service = service
        let computation = Task.detached(priority: .userInitiated) { try Task.checkCancellation(); return try operation(service) }
        return try await withTaskCancellationHandler { try await computation.value } onCancel: { computation.cancel() }
    }

    func projectNavigation(_ path: NavigationPath, in navigation: NavigationPathState) {
        let scopeId = actionNavigation.flatMap { $0.owner === navigation ? $0.scopeId : nil }
        if let scopeId {
            removeActionNavigation(scopeId: scopeId)
        }
        navigation.binding.wrappedValue = path
        guard let scopeId, let scope = activeScope, scope.id() == scopeId, !isCancelled else { return }
        guard navigation.path == path else {
            needsReload = true
            invalidateExecution(clearPath: false)
            return
        }
        observeActionNavigation(scope: scope, navigation: navigation)
    }

    private func observeActionNavigation(scope: BsvHomeBsvColdWalletActionScope, navigation: NavigationPathState) {
        let scopeId = scope.id()
        let observerId = navigation.observeRemoval { [weak self] in
            guard let self, activeScope?.id() == scopeId else { return }
            needsReload = true
            invalidateExecution(clearPath: false)
        }
        actionNavigation = (scopeId, navigation, observerId)
    }

    private func removeActionNavigation(scopeId: String) {
        guard let entry = actionNavigation, entry.scopeId == scopeId else { return }
        actionNavigation = nil
        entry.owner.removeRemovalObserver(entry.observerId)
    }

    private func financial(walletId: String, intent: BsvHomeBsvColdWalletActionIntent, navigation: NavigationPathState, action: @escaping @MainActor (UUID, BsvHomeBsvColdWalletActionScope) async throws -> Void) {
        guard !isCancelled, !isBusy, isForeground, !isCaptured else { return }
        do {
            let scope = try service.beginAction(walletId: walletId, intent: intent)
            activeScope = scope
            observeActionNavigation(scope: scope, navigation: navigation)
            actionOutcome = nil
            operation(scope: scope) { context, _ in
                do { try await action(context, scope) }
                catch {
                    try self.checkContext(context)
                    self.actionOutcome = try scope.outcome()
                    self.isReviewConfirmed = false
                    if self.actionOutcome?.submission == .submitted {
                        self.path = [.result]
                        self.errorMessage = Localized.coldWalletBroadcastStorageFailed
                    } else if self.actionOutcome?.submission == .unknown {
                        self.path = [.broadcast]
                        self.errorMessage = Localized.coldWalletBroadcastRetry
                    }
                    if self.actionOutcome?.requiresReload == true {
                        self.requiresStateReload = true
                        do {
                            let snapshot = try await self.service.snapshot(walletId: walletId)
                            try self.checkContext(context)
                            self.apply(snapshot)
                            self.requiresStateReload = false
                        } catch { try self.checkContext(context); self.loadFailed = true }
                    }
                    throw error
                }
            }
        } catch { errorMessage = Localized.coldWalletStorageError }
    }

    @discardableResult
    private func operation(scope: BsvHomeBsvColdWalletActionScope? = nil, _ action: @escaping @MainActor (UUID, UInt64) async throws -> Void) -> Task<Void, Never>? {
        guard !isCancelled, !isBusy, isForeground, !isCaptured else { scope?.cancel(); return nil }
        let generation = service.captureGeneration()
        isBusy = true
        errorMessage = nil
        loadFailed = false
        let context = contextId
        let task = Task {
            do {
                try self.checkContext(context)
                try await action(context, generation)
            } catch is CancellationError {}
            catch {
                if context == self.contextId, self.errorMessage == nil {
                    if let serviceError = error as? BsvHomeServiceError {
                        switch serviceError {
                        case .Cancelled: break
                        case .Store: self.errorMessage = Localized.coldWalletStorageError
                        case .Platform: self.errorMessage = Localized.multisigErrorAuthentication
                        default: self.errorMessage = self.path.last == .broadcast ? Localized.coldWalletBroadcastRetry : Localized.coldWalletInvalidData
                        }
                    } else {
                        self.errorMessage = Localized.coldWalletStorageError
                    }
                }
            }
            if context == self.contextId {
                if let scope, self.activeScope?.id() == scope.id() {
                    self.removeActionNavigation(scopeId: scope.id())
                    self.activeScope = nil
                    scope.cancel()
                    self.cancelAuthorization(scope.id())
                }
                self.isBusy = false
                self.operationTask = nil
            }
        }
        operationTask = task
        return task
    }

    private func applyEntry(context: UUID, generation: UInt64) async throws {
        switch entry {
        case .home: break
        case let .scanRequest(presentation):
            path = [.workspaceHome, .signing]
            try checkContext(context)
            initialRequestScan = presentation == .initial && isSigner ? .pending : .consumed
        case .create:
            guard canHoldSecrets else { throw BsvHomeServiceError.Cancelled }
            let words = try await local { try $0.createWords(expectedGeneration: generation) }
            try checkContext(context)
            clearSecrets()
            self.words = words
            importIsSecret = true
            path = [.create]
        case .recover: importIsSecret = true; path = [.recover]
        case .connect: importIsSecret = false; path = [.connect]
        case .signing: path = [.signing]
        case .pairing:
            if isSigner {
                try await pairingFrames(context: context); path = [.pairing]
            } else {
                guard preferredWalletId == nil else { throw BsvHomeServiceError.Cancelled }
                path = [.signing]
            }
        case .send:
            if isObserver {
                path = [.send]
            } else {
                errorMessage = Localized.coldWalletConnectRequired
            }
        case .receive:
            if isObserver {
                path = [.receive]
            } else {
                errorMessage = Localized.coldWalletConnectRequired
            }
        }
    }

    private func exportPairingSnapshot(walletId: String, context: UUID, generation: UInt64) async throws {
        let snapshot = try await service.exportPairing(walletId: walletId, expectedGeneration: generation)
        try checkContext(context)
        apply(snapshot)
        let candidate = BsvHomeBsvColdWalletCandidate(wallet: snapshot.wallet, role: snapshot.role)
        if let index = candidates.firstIndex(where: { $0.wallet.id == snapshot.wallet.id }) {
            candidates[index] = candidate
        } else {
            candidates.append(candidate)
        }
        wallets = candidates.map(\.wallet)
        try await loadSecretKind(snapshot.wallet.id, context: context)
    }

    private func pairingFrames(context: UUID) async throws {
        guard let snapshot, isSigner else { throw BsvHomeServiceError.Cancelled }
        frames = try await local { try $0.qrFrames(payloadJson: snapshot.pairingJson) }
        try checkContext(context)
    }

    func scanRecipient(using scanner: BsvColdWalletRecipientScanner) {
        guard canPrepare, let wallet, path.last == .send else { return }
        let ticket = UUID()
        let context = contextId
        recipientScannerId = ticket
        scanner { [weak self] value in
            guard let self, !self.isCancelled, recipientScannerId == ticket, contextId == context,
                  self.wallet?.id == wallet.id, path.last == .send, !self.isBusy, isForeground, !self.isCaptured else { return }
            recipientScannerId = nil
            do {
                let payment = try bsvMultisigScanRecipient(value: value)
                recipient = payment.recipient.address
                if let amount = payment.amount {
                    self.amount = amount.replacingOccurrences(of: ".", with: Locale.current.decimalSeparator ?? ".")
                }
            } catch { errorMessage = Localized.coldWalletInvalidData }
        }
    }
}
