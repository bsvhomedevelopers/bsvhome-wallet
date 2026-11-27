import BsvMultisig
import func BsvHomeCore.bsvMultisigOverview
import func BsvHomeCore.bsvMultisigSigningWallets
import enum BsvHomeCore.BsvHomeBsvMultisigError
import struct BsvHomeCore.BsvHomeBsvMultisigIdentitySnapshot
import struct BsvHomeCore.BsvHomeBsvMultisigOverview
import struct BsvHomeCore.BsvHomeBsvMultisigWallet
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import Localization
import Observation
import Primitives

@Observable
@MainActor
final class CollectiveWalletDirectory {
    private struct Entry {
        let wallet: Primitives.Wallet
        let model: BsvMultisigViewModel
    }

    private let session: any BsvHomeWalletSessionServiceProtocol
    private let loadWallets: @Sendable () async throws -> [BsvHomeCore.Wallet]
    private let makeModel: @MainActor (Primitives.Wallet) -> BsvMultisigViewModel
    private let preferences: any BsvHomePreferencesServiceProtocol
    private let origin: String
    private var entries: [Primitives.WalletId: Entry] = [:]
    private var orderedEntries: [Entry] = []
    private var preferredSignerWalletId: String?
    private var selectedSigners: [String: String] = [:]
    private var invalidatedSigners: Set<Primitives.WalletId> = []
    private var sourceErrorMessage: String?
    private var revision: UInt = 0
    private var refreshTask: Task<Void, Never>?

    private(set) var isLoading = false
    let authorization: BsvMultisigPasswordAuthorization?

    init(session: any BsvHomeWalletSessionServiceProtocol, loadWallets: @escaping @Sendable () async throws -> [BsvHomeCore.Wallet], preferences: any BsvHomePreferencesServiceProtocol, origin: String, authorization: BsvMultisigPasswordAuthorization? = nil, makeModel: @escaping @MainActor (Primitives.Wallet) -> BsvMultisigViewModel) {
        self.session = session
        self.loadWallets = loadWallets
        self.makeModel = makeModel
        self.preferences = preferences
        self.origin = origin
        self.authorization = authorization
    }

    var overview: BsvHomeBsvMultisigOverview {
        let identities = orderedEntries.map { entry in
            BsvHomeBsvMultisigIdentitySnapshot(signerWalletId: entry.wallet.id.id, wallets: entry.model.wallets, proposals: entry.model.proposals)
        }
        return bsvMultisigOverview(identities: identities, preferredSigners: selectedSigners, fallbackSigner: preferredSignerWalletId)
    }

    var wallets: [BsvHomeBsvMultisigWallet] { overview.wallets }

    var signingWallets: [Primitives.Wallet] {
        orderedEntries.map(\.wallet)
    }

    func cancelActions() {
        for entry in entries.values {
            entry.model.cancelActions()
        }
        authorization?.cancel()
    }

    func identities(for walletId: String?) -> [Primitives.Wallet] {
        guard let walletId else { return signingWallets }
        return orderedEntries.filter { entry in
            entry.model.wallets.contains { $0.id == walletId && $0.localSignerWalletId == entry.wallet.id.id }
        }.map(\.wallet)
    }

    func selectSigner(_ signerWalletId: Primitives.WalletId, for walletId: String) throws {
        guard identities(for: walletId).contains(where: { $0.id == signerWalletId }) else {
            throw BsvHomeBsvMultisigError.MissingSigner
        }
        let previous = wallets.first { $0.id == walletId }?.localSignerWalletId
        try preferences.setMultisigSigner(origin: origin, walletId: walletId, signerWalletId: signerWalletId.id)
        selectedSigners[walletId] = signerWalletId.id
        if previous != signerWalletId.id {
            if let entry = orderedEntries.first(where: { $0.wallet.id.id == previous }) {
                invalidatedSigners.insert(entry.wallet.id)
                entry.model.cancel()
            }
        }
    }

    var errorMessage: String? {
        let identityErrors = orderedEntries.compactMap { entry -> String? in
            guard let error = entry.model.errorMessage else { return nil }
            let name = entry.wallet.name.isEmpty ? entry.wallet.id.id : entry.wallet.name
            return "\(name): \(error)"
        }
        let messages = [sourceErrorMessage].compactMap(\.self) + identityErrors
        return messages.isEmpty ? nil : messages.joined(separator: "\n")
    }

    func model(for walletId: Primitives.WalletId) -> BsvMultisigViewModel? {
        entries[walletId]?.model
    }

    func refresh() async {
        guard !Task.isCancelled else { return }
        if let refreshTask {
            await refreshTask.value
            return
        }
        let currentRevision = revision
        isLoading = true
        defer {
            if revision == currentRevision {
                refreshTask = nil
                isLoading = false
            }
        }
        let task = Task { await performRefresh(revision: currentRevision) }
        refreshTask = task
        await task.value
    }

    private func performRefresh(revision currentRevision: UInt) async {
        do {
            try Task.checkCancellation()
            let storedWallets = try await loadWallets()
            try Task.checkCancellation()
            guard revision == currentRevision else { return }
            let signingWallets = try bsvMultisigSigningWallets(wallets: storedWallets).map { $0.map() }
            let preferred = try session.getCurrentWalletId()
            var updated: [Primitives.WalletId: Entry] = [:]
            var ordered: [Entry] = []
            for wallet in signingWallets {
                let next: Entry
                if let entry = entries[wallet.id], entry.wallet == wallet, !invalidatedSigners.contains(wallet.id) {
                    next = entry
                } else {
                    entries[wallet.id]?.model.cancel()
                    next = Entry(wallet: wallet, model: makeModel(wallet))
                    invalidatedSigners.remove(wallet.id)
                }
                updated[wallet.id] = next
                ordered.append(next)
            }
            for (id, entry) in entries where updated[id] == nil {
                entry.model.cancel()
            }
            entries = updated
            orderedEntries = ordered
            preferredSignerWalletId = preferred
            sourceErrorMessage = nil

            await withTaskGroup(of: Void.self) { group in
                for entry in ordered {
                    let model = entry.model
                    group.addTask { await model.refresh() }
                }
            }
            try Task.checkCancellation()
            guard revision == currentRevision else { return }
            preferredSignerWalletId = try session.getCurrentWalletId()
            var restored: [String: String] = [:]
            for id in Set(orderedEntries.flatMap(\.model.wallets).map(\.id)) {
                restored[id] = try preferences.multisigSigner(origin: origin, walletId: id)
            }
            selectedSigners = restored
        } catch is CancellationError {
            guard revision == currentRevision else { return }
            cancel()
        } catch {
            guard revision == currentRevision else { return }
            if Task.isCancelled {
                cancel()
                return
            }
            discardModels()
            sourceErrorMessage = error is BsvHomeBsvMultisigError ? Localized.multisigErrorMissingSigner : Localized.multisigErrorStorage
        }
    }

    func cancel() {
        cancelActions()
        revision &+= 1
        refreshTask?.cancel()
        refreshTask = nil
        discardModels()
        sourceErrorMessage = nil
        isLoading = false
    }

    private func discardModels() {
        for entry in entries.values {
            entry.model.cancel()
        }
        entries = [:]
        orderedEntries = []
        preferredSignerWalletId = nil
        selectedSigners = [:]
        invalidatedSigners = []
    }
}
