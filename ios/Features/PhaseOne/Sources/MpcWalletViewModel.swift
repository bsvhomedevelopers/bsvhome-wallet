import Foundation
import struct BsvHomeCore.BsvHomeMpcBackupMaterial
import enum BsvHomeCore.BsvHomeMpcError
import struct BsvHomeCore.BsvHomeMpcHistoryPage
import struct BsvHomeCore.BsvHomeMpcPaymailSnapshot
import struct BsvHomeCore.BsvHomeMpcPendingPreparation
import struct BsvHomeCore.BsvHomeMpcPendingTask
import struct BsvHomeCore.BsvHomeMpcRecovery
import struct BsvHomeCore.BsvHomeMpcRecoveryInformation
import struct BsvHomeCore.BsvHomeMpcTransfer
import struct BsvHomeCore.BsvHomeMpcWallet
import protocol BsvHomeCore.BsvHomeMpcWalletServiceProtocol
import struct BsvHomeCore.BsvHomePaymentRecipient
import Localization
import Observation

@Observable
@MainActor
public final class MpcWalletViewModel {
    private let service: any BsvHomeMpcWalletServiceProtocol
    private let authenticate: @MainActor @Sendable () async throws -> Void
    private let onWalletChange: @MainActor @Sendable () async throws -> Void
    private let onSessionInvalidated: @MainActor @Sendable () async -> Void
    public private(set) var selectedWalletId: String?
    private var activeExportId: UUID?
    private var backupPageOwner: UUID?
    private enum ContextTransition {
        case ready
        case pending(Task<Void, Error>)
        case retry
    }

    private var contextTransition = ContextTransition.ready
    public private(set) var wallets: [BsvHomeMpcWallet] = []
    var transfer: BsvHomeMpcTransfer?
    var history: BsvHomeMpcHistoryPage?
    var pendingPreparation: BsvHomeMpcPendingPreparation?
    var pendingTask: BsvHomeMpcPendingTask?
    var recovery: BsvHomeMpcRecovery?
    var timeline: BsvHomeMpcRecoveryInformation?
    var paymail: BsvHomeMpcPaymailSnapshot?
    var needsPaymailSetup = false
    var backup: BsvHomeMpcBackupMaterial?
    var savedCommitment: String?
    var recoveryKeyConfirmation = ""
    var importedDocument: String?
    var importRecoveryKey = ""
    var name = ""
    var recipient = ""
    private(set) var recipientName: String?
    private var namedRecipientAddress: String?
    var amount = ""
    var handle = ""
    var reason = ""
    private(set) var isBusy = false
    private(set) var error: String?
    private var revision: UInt = 0
    private var preparationKey: String?

    public init(service: any BsvHomeMpcWalletServiceProtocol, authenticate: @escaping @MainActor @Sendable () async throws -> Void, onWalletChange: @escaping @MainActor @Sendable () async throws -> Void, onSessionInvalidated: @escaping @MainActor @Sendable () async -> Void = {}) {
        self.service = service
        self.authenticate = authenticate
        self.onWalletChange = onWalletChange
        self.onSessionInvalidated = onSessionInvalidated
    }

    private func authenticateLocally() async -> Bool {
        let current = revision
        do { try await authenticate(); return current == revision }
        catch { self.error = Localized.phaseoneCancelled; return false }
    }

    public func load() async {
        if let cached = await value({ try $0.localWallets() }) {
            wallets = cached
        }
        if let pending = await value({ try $0.pendingPreparation() }) {
            pendingPreparation = pending
            if let pending, pending.walletId == selectedWalletId, recipient.isEmpty, amount.isEmpty {
                restorePendingDraft(pending.walletId)
            }
        }
        if let task = await value({ try $0.pendingTask() }) {
            pendingTask = task
        }
        if let wallets = await value({ try await $0.listWallets() }) {
            self.wallets = wallets
        }
    }

    func reconcile() async {
        _ = await value { try await $0.reconcile() }
        await load()
    }

    func cancelTask() async {
        _ = await value { try await $0.cancelTask() }
        await load()
    }

    func create() async -> String? {
        guard await authenticateLocally() else { return nil }
        let name = name
        guard let wallet = await value({ try await $0.createWallet(name: name) }) else { return nil }
        self.name = ""
        selectWallet(wallet.walletId)
        await load()
        return wallet.walletId
    }

    func exportBackup(_ id: String) async {
        guard id == selectedWalletId else { error = BsvHomeMpcError.IdentityMismatch.phaseOneMessage; return }
        clearBackup()
        guard await authenticateLocally() else { return }
        if let material = await value({ try await $0.exportBackup(walletId: id) }) {
            backup = material
        }
    }

    func makeBackupExport() async -> MpcBackupExportTicket? {
        guard !isBusy, let material = backup, material.walletId == selectedWalletId else { return nil }
        isBusy = true
        defer { isBusy = false }
        let current = revision
        let environment = material.environment
        let ownerId = material.ownerId
        let walletId = material.walletId
        let epoch = material.epoch
        let commitment = material.commitment
        let document = material.document
        do {
            let ticket = try await Task.detached(priority: .userInitiated) {
                try MpcBackupExportTicket.create(revision: current, environment: environment, ownerId: ownerId, walletId: walletId, epoch: epoch, commitment: commitment, document: document)
            }.value
            guard let material = backup, ticket.matches(material, revision: revision, walletId: selectedWalletId) else {
                try ticket.removeTemporaryFile()
                return nil
            }
            activeExportId = ticket.id
            return ticket
        } catch {
            if current == revision {
                self.error = Localized.phaseoneStorageUnavailable
            }
            return nil
        }
    }

    func isCurrentExport(_ ticket: MpcBackupExportTicket) -> Bool {
        guard activeExportId == ticket.id, let backup else { return false }
        return ticket.matches(backup, revision: revision, walletId: selectedWalletId)
    }

    func discardBackupExport(_ ticket: MpcBackupExportTicket) {
        do { try ticket.removeTemporaryFile() }
        catch {
            if isCurrentExport(ticket) {
                self.error = Localized.phaseoneStorageUnavailable
            }
        }
    }

    func backupExportFinished(_ ticket: MpcBackupExportTicket, outcome: MpcBackupExportOutcome, cleanupFailed: Bool) {
        guard isCurrentExport(ticket) else { return }
        activeExportId = nil
        switch outcome {
        case .saved:
            savedCommitment = ticket.commitment
        case .cancelled:
            error = Localized.phaseoneOfflineSaveCancelled
        case .abandoned:
            clearSensitive()
        }
        if cleanupFailed {
            error = Localized.phaseoneStorageUnavailable
        }
    }

    func confirmBackup() async {
        guard let backup, backup.walletId == selectedWalletId, savedCommitment == backup.commitment else { return }
        let confirmation = recoveryKeyConfirmation
        if await value({ try await $0.confirmBackup(material: backup, recoveryKeyConfirmation: confirmation, fileSaved: true) }) != nil {
            clearBackup()
            await load()
        }
    }

    func clearBackup() {
        activeExportId = nil
        backup = nil
        savedCommitment = nil
        recoveryKeyConfirmation = ""
    }

    func readBackup(_ url: URL) async {
        let current = revision
        do {
            let data = try await Task.detached(priority: .userInitiated) {
                let access = url.startAccessingSecurityScopedResource()
                defer {
                    if access {
                        url.stopAccessingSecurityScopedResource()
                    }
                }
                let handle = try FileHandle(forReadingFrom: url)
                defer { try? handle.close() }
                let data = try handle.read(upToCount: 4_194_305) ?? Data()
                guard data.count <= 4_194_304, let document = String(data: data, encoding: .utf8) else { throw BsvHomeMpcError.InvalidInput }
                return document
            }.value
            guard current == revision else { return }
            importedDocument = data
        } catch { self.error = Localized.phaseoneBackupMismatch }
    }

    func restore() async -> String? {
        guard let document = importedDocument, await authenticateLocally() else { return nil }
        let key = importRecoveryKey
        let name = name
        guard let wallet = await value({ try await $0.restoreBackup(document: document, recoveryKey: key, name: name) }) else { return nil }
        importedDocument = nil
        importRecoveryKey = ""
        self.name = ""
        selectWallet(wallet.walletId)
        await load()
        return wallet.walletId
    }

    func refreshWallet(_ id: String) async {
        if let wallet = await value({ try await $0.refreshWallet(walletId: id) }) {
            wallets.removeAll { $0.walletId == wallet.walletId }
            wallets.append(wallet)
        }
    }

    func recipientInputCallback(walletId: String) -> (@MainActor @Sendable (BsvHomePaymentRecipient) -> Bool)? {
        guard walletId == selectedWalletId, !isBusy else { return nil }
        let current = revision
        return { [weak self] payment in
            guard let self, revision == current, selectedWalletId == walletId else { return false }
            draftChanged()
            recipient = payment.recipient.address
            recipientName = payment.recipient.name
            namedRecipientAddress = payment.recipient.address
            if let amount = payment.amount {
                self.amount = amount
            }
            return true
        }
    }

    func draftChanged() {
        revision &+= 1
        if recipient != namedRecipientAddress {
            recipientName = nil; namedRecipientAddress = nil
        }
        transfer = nil
        preparationKey = nil
    }

    func prepare(_ id: String) async -> String? {
        guard id == selectedWalletId else { error = BsvHomeMpcError.IdentityMismatch.phaseOneMessage; return nil }
        let recipient = recipient
        let amount = amount.replacingOccurrences(of: Locale.current.decimalSeparator ?? ".", with: ".")
        let key = preparationKey ?? UUID().uuidString
        preparationKey = key
        guard let transfer = await value({ try await $0.prepareTransferBsv(walletId: id, recipient: recipient, amountBsv: amount, idempotencyKey: key) }) else { return nil }
        self.transfer = transfer
        return transfer.id
    }

    func refreshTransfer(_ id: String) async {
        if let transfer = await value({ try await $0.refreshTransfer(id: id) }) {
            selectWallet(transfer.walletId); self.transfer = transfer
        }
    }

    func signTransfer() async {
        guard let transfer, transfer.walletId == selectedWalletId else { error = BsvHomeMpcError.IdentityMismatch.phaseOneMessage; return }
        if let result = await value({ try await $0.signTransfer(id: transfer.id, confirmedCommitment: transfer.review.commitment) }) {
            self.transfer = result
        }
    }

    func broadcastTransfer() async {
        guard let transfer, transfer.walletId == selectedWalletId else { error = BsvHomeMpcError.IdentityMismatch.phaseOneMessage; return }
        if let result = await value({ try await $0.broadcastTransfer(id: transfer.id, confirmedCommitment: transfer.review.commitment) }) {
            self.transfer = result
        }
    }

    func cancelTransfer() async {
        guard let transfer else { return }
        if let result = await value({ try await $0.cancelTransfer(id: transfer.id) }) {
            self.transfer = result
        }
    }

    func loadHistory(_ id: String, page: UInt32) async {
        if let history = await value({ try await $0.transactionHistory(walletId: id, page: page) }) {
            self.history = history
        }
    }

    func bindPaymail(_ id: String, active: Bool) async {
        let handle = handle
        if await value({ try await $0.bindPaymail(walletId: id, handle: handle, active: active) }) != nil {
            needsPaymailSetup = false
            await loadPaymail(id)
        }
    }

    func loadPaymail(_ id: String) async {
        if let snapshot = await value({ try await $0.paymailSnapshot(walletId: id) }) {
            paymail = snapshot
        }
    }

    func startRecovery(_ id: String) async {
        if let recovery = await value({ try await $0.startRecovery(walletId: id) }) {
            if self.recovery?.id != recovery.id {
                timeline = nil
            }
            self.recovery = recovery
        }
    }

    func loadRecovery() async {
        if let recovery = await value({ try await $0.currentRecovery() }) {
            self.recovery = recovery
        }
    }

    func refreshRecovery() async {
        guard let recovery else { return }
        if let result = await value({ try await $0.recoveryStatus(id: recovery.id) }) {
            self.recovery = result
        }
        if let timeline = await value({ try await $0.recoveryTimeline(id: recovery.id) }) {
            self.timeline = timeline
        }
    }

    func continueRecovery() async {
        guard let recovery else { return }
        if let result = await value({ try await $0.refreshRecovery(id: recovery.id) }) {
            self.recovery = result; await load()
        }
    }

    func cancelRecovery() async {
        guard let recovery else { return }
        if let result = await value({ try await $0.cancelRecovery(id: recovery.id) }) {
            self.recovery = result; await load()
        }
    }

    func supplyInformation() async {
        guard let recovery else { return }
        let reason = reason
        if await value({ try await $0.submitRecoveryInformation(id: recovery.id, reason: reason) }) != nil {
            self.reason = ""; await refreshRecovery()
        }
    }

    public func selectWallet(_ id: String?) {
        guard selectedWalletId != id else { return }
        let previous = selectedWalletId
        selectedWalletId = id
        clearSensitive()
        transfer = nil
        history = nil
        paymail = nil
        preparationKey = nil
        recipient = ""
        recipientName = nil
        namedRecipientAddress = nil
        amount = ""
        handle = ""
        reason = ""
        if let previous, previous != id {
            let transition = onWalletChange
            contextTransition = .pending(Task { try await transition() })
        }
        if let id {
            restorePendingDraft(id)
        }
    }

    func restorePendingDraft(_ id: String) {
        guard let pendingPreparation, pendingPreparation.walletId == id, selectedWalletId == id else { return }
        revision &+= 1
        transfer = nil
        recipientName = nil
        namedRecipientAddress = nil
        recipient = pendingPreparation.recipient
        amount = (Decimal(pendingPreparation.amount) / Decimal(100_000_000)).formatted(.number.grouping(.never).precision(.fractionLength(0 ... 8)))
        preparationKey = pendingPreparation.key
    }

    func leaveBackupPage(_ id: String) {
        if backup?.walletId == id {
            clearSensitive()
        }
    }

    func enterBackupPage(_ owner: UUID) {
        if let current = backupPageOwner, current != owner {
            clearSensitive()
        }
        backupPageOwner = owner
    }

    @discardableResult
    func leaveBackupPage(_ id: String, owner: UUID) -> Bool {
        guard backupPageOwner == owner else { return false }
        backupPageOwner = nil
        leaveBackupPage(id)
        return true
    }

    public func clearSession() {
        clearSensitive()
        selectedWalletId = nil
        backupPageOwner = nil
        wallets = []
        transfer = nil
        history = nil
        recovery = nil
        timeline = nil
        paymail = nil
        needsPaymailSetup = false
        error = nil
        pendingPreparation = nil
        pendingTask = nil
        preparationKey = nil
        recipient = ""
        recipientName = nil
        namedRecipientAddress = nil
        amount = ""
        handle = ""
        reason = ""
    }

    public func clearSensitive() {
        revision &+= 1
        clearBackup()
        importedDocument = nil
        importRecoveryKey = ""
    }

    func value<T: Sendable>(_ call: @escaping @Sendable (any BsvHomeMpcWalletServiceProtocol) async throws -> T) async -> T? {
        guard !Task.isCancelled, !isBusy else { return nil }
        isBusy = true
        error = nil
        let current = revision
        let service = service
        defer { isBusy = false }
        do {
            if case .retry = contextTransition {
                let transition = onWalletChange
                contextTransition = .pending(Task { try await transition() })
            }
            if case let .pending(transition) = contextTransition {
                do { try await transition.value }
                catch {
                    if current == revision {
                        contextTransition = .retry
                    }
                    throw BsvHomeMpcError.SecureStorageUnavailable
                }
                guard current == revision, !Task.isCancelled else { return nil }
                contextTransition = .ready
            }
            let result = try await Task.detached(priority: .userInitiated) { try await call(service) }.value
            guard current == revision else { return nil }
            return result
        } catch {
            if let failure = error as? BsvHomeMpcError, case let .Platform(platformError) = failure, platformError == .LoginRequired || platformError == .SessionChanged {
                if current == revision {
                    clearSensitive()
                    self.error = failure.phaseOneMessage
                }
                await onSessionInvalidated()
                return nil
            }
            guard current == revision else { return nil }
            needsPaymailSetup = (error as? BsvHomeMpcError) == .PaymailSenderRequired
            self.error = (error as? BsvHomeMpcError)?.phaseOneMessage ?? Localized.phaseoneServiceError
            return nil
        }
    }

    static func bsv(_ value: UInt64?) -> String {
        guard let value else { return "—" }
        return (Decimal(value) / Decimal(100_000_000)).formatted(.number.precision(.fractionLength(0 ... 8))) + " BSV"
    }
}
