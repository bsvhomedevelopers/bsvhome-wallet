import BsvMultisig
import Components
import Foundation
import struct BsvHomeCore.BsvHomeBsvMultisigFundingPayer
import Observation
import Primitives
import SwiftUI

@Observable
@MainActor
final class BsvPublicFundingContext: Identifiable {
    let id = UUID()
    let entry: BsvMultisigPublicFundingEntry
    let navigation: NavigationPathState
    let authorization: BsvMultisigPasswordAuthorization
    private let loadCandidates: @MainActor () async throws -> (wallets: [Wallet], payers: [BsvHomeBsvMultisigFundingPayer])
    private let makeModel: @MainActor (Wallet) -> BsvMultisigPublicFundingViewModel
    private var wallets: [Wallet] = []
    private var selectedWallet: Wallet?
    private(set) var loadTask: Task<Void, Never>?
    private var revision: UInt = 0
    private var retired = false
    private var previousPath: NavigationPath?
    private var entryPath: NavigationPath?
    private var removalObserver: UUID?
    private var onExit: (@MainActor () -> Void)?
    private(set) var eligiblePayers: StateViewType<[BsvHomeBsvMultisigFundingPayer]> = .noData
    private(set) var model: BsvMultisigPublicFundingViewModel?

    init(
        entry: BsvMultisigPublicFundingEntry,
        navigation: NavigationPathState,
        authorization: BsvMultisigPasswordAuthorization,
        loadCandidates: @escaping @MainActor () async throws -> (wallets: [Wallet], payers: [BsvHomeBsvMultisigFundingPayer]),
        makeModel: @escaping @MainActor (Wallet) -> BsvMultisigPublicFundingViewModel,
    ) {
        self.entry = entry
        self.navigation = navigation
        self.authorization = authorization
        self.loadCandidates = loadCandidates
        self.makeModel = makeModel
    }

    func present(onExit: @escaping @MainActor () -> Void) {
        guard !retired, previousPath == nil else { return }
        previousPath = navigation.path
        self.onExit = onExit
        navigation.append(BsvPublicFundingDestination(id: id))
        entryPath = navigation.path
        removalObserver = navigation.observeRemoval { [weak self] in
            self?.retire()
        }
    }

    func loadPayers() {
        guard !retired, loadTask == nil else { return }
        eligiblePayers = .loading
        let current = revision
        loadTask = Task { [weak self] in
            guard let self, !retired, !Task.isCancelled, current == revision else { return }
            defer {
                if current == revision {
                    loadTask = nil
                }
            }
            do {
                let values = try await loadCandidates()
                guard !retired, !Task.isCancelled, current == revision else { return }
                if let selectedWallet, !values.wallets.contains(selectedWallet) || !values.payers.contains(where: { $0.walletId == selectedWallet.id.id }) {
                    clearPayer()
                    popToEntry()
                }
                wallets = values.wallets
                eligiblePayers = .data(values.payers)
            } catch {
                guard !retired, !Task.isCancelled, current == revision else { return }
                clearPayer()
                popToEntry()
                wallets = []
                eligiblePayers = .error(error)
            }
        }
    }

    func selectPayer(_ payer: BsvHomeBsvMultisigFundingPayer) {
        guard !retired, let candidates = eligiblePayers.value, candidates.contains(payer),
              let wallet = wallets.first(where: { $0.id.id == payer.walletId }), wallet != selectedWallet
        else { return }
        clearPayer()
        popToEntry()
        selectedWallet = wallet
        model = makeModel(wallet)
    }

    func dismiss() {
        guard !retired else { return }
        let previousPath = previousPath
        retire()
        if let previousPath {
            navigation.binding.wrappedValue = previousPath
        }
    }

    private func retire() {
        guard !retired else { return }
        retired = true
        if let removalObserver {
            navigation.removeRemovalObserver(removalObserver)
        }
        removalObserver = nil
        revision &+= 1
        loadTask?.cancel()
        loadTask = nil
        clearPayer()
        wallets = []
        if case let .payment(source) = entry {
            source.cancelPaymailPreparation()
        }
        let onExit = onExit
        self.onExit = nil
        onExit?()
    }

    private func clearPayer() {
        model?.cancel()
        model = nil
        selectedWallet = nil
        authorization.cancel()
    }

    private func popToEntry() {
        if let entryPath {
            navigation.binding.wrappedValue = entryPath
        }
    }
}
