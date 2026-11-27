import BsvMultisig
import Components
import Foundation
import Localization
import Onboarding
import Primitives
import SwiftUI

@Observable
@MainActor
final class CollectiveOnboardingModel: Identifiable {
    enum Entry: Equatable { case create, importChoice, join(invitation: String), list }
    enum Lifetime { case onboarding, overview }
    enum IdentityCompletion { case created, recovered }

    struct IdentityModal: Identifiable {
        enum Content {
            case create(CreateWalletModel)
            case recovery(ImportWalletViewModel)
        }

        let id: UUID
        let content: Content
    }

    let id = UUID()
    private(set) var entry: Entry
    let directory: CollectiveWalletDirectory
    let lifetime: Lifetime
    let authorization: BsvMultisigPasswordAuthorization
    private let factory: ViewModelFactory
    private let onIdentityComplete: @MainActor (Wallet, IdentityCompletion) -> Void
    private var revision: UInt = 0
    private var isActive = false
    private var loadingTask: Task<Void, Never>?
    private var boundWallet: Wallet?
    private(set) var scopeId = UUID()
    private(set) var selectedSignerWalletId: WalletId?
    private(set) var featureModel: BsvMultisigViewModel?
    private(set) var identityModal: IdentityModal?
    let scanner = BsvMultisigScannerPresentation()
    private(set) var expectedCurrentWalletId: WalletId?
    private(set) var isSelecting = false
    var errorMessage: String?

    init(entry: Entry, directory: CollectiveWalletDirectory, factory: ViewModelFactory, signerWalletId: WalletId? = nil, lifetime: Lifetime = .onboarding, onIdentityComplete: @escaping @MainActor (Wallet, IdentityCompletion) -> Void = { _, _ in }) {
        self.entry = entry
        self.directory = directory
        self.factory = factory
        authorization = factory.bsvMultisigPasswordAuthorization()
        self.lifetime = lifetime
        selectedSignerWalletId = signerWalletId
        self.onIdentityComplete = onIdentityComplete
    }

    func start(afterUnlock: @escaping @MainActor () async -> Void) {
        guard !isActive else { return }
        isActive = true
        scopeId = UUID()
        refresh(afterUnlock: afterUnlock)
    }

    func refresh(afterUnlock: @escaping @MainActor () async -> Void) {
        guard isActive, loadingTask == nil else { return }
        let currentRevision = revision
        isSelecting = true
        loadingTask = Task {
            defer {
                if revision == currentRevision {
                    loadingTask = nil
                    isSelecting = false
                }
            }
            await afterUnlock()
            guard isCurrent(currentRevision) else { return }
            await directory.refresh()
            guard isCurrent(currentRevision) else { return }
            let candidates = directory.signingWallets
            let selected = candidates.first { $0.id == selectedSignerWalletId }
                ?? candidates.first { $0.id == factory.walletSessionService.currentWalletId }
                ?? candidates.first
            guard let selected else {
                scanner.cancel()
                featureModel?.cancel()
                featureModel = nil
                boundWallet = nil
                errorMessage = directory.errorMessage
                return
            }
            if featureModel == nil || boundWallet != selected {
                scanner.cancel()
                featureModel?.cancel()
                selectedSignerWalletId = selected.id
                featureModel = factory.bsvMultisigScene(wallet: selected, authorization: authorization, cancelAuthorization: authorization.cancel(scopeId:))
                boundWallet = selected
            }
            await featureModel?.load()
            guard isCurrent(currentRevision) else { return }
            errorMessage = directory.errorMessage ?? featureModel?.errorMessage
        }
    }

    func context(for route: BsvMultisigRoute, onSelect: @escaping @MainActor @Sendable (String) -> Void) -> BsvMultisigIdentityContext {
        let options = directory.identities(for: route.walletId(in: featureModel)).map { wallet in
            BsvMultisigIdentityOption(
                walletId: wallet.id.id,
                name: wallet.name,
                address: wallet.accounts.first { $0.chain == .bitcoinSv }?.address ?? "",
                isDedicated: factory.walletService.isMultisigIdentity(walletId: wallet.id.id),
            )
        }
        return BsvMultisigIdentityContext(options: options, selectedWalletId: selectedSignerWalletId?.id, isSelecting: isSelecting, onSelect: onSelect, onCreateDedicated: { [weak self] _ in self?.presentDedicatedCreation() })
    }

    func selectSigner(_ id: String, for route: BsvMultisigRoute, afterUnlock: @escaping @MainActor () async -> Void) {
        guard let wallet = directory.identities(for: route.walletId(in: featureModel)).first(where: { $0.id.id == id }), wallet.id != selectedSignerWalletId else { return }
        do {
            if let remoteWalletId = route.walletId(in: featureModel) {
                try directory.selectSigner(wallet.id, for: remoteWalletId)
            }
            replaceFeature(with: wallet)
            refresh(afterUnlock: afterUnlock)
        } catch {
            errorMessage = Localized.multisigErrorStorage
        }
    }

    func scanInvitation(navigation: NavigationPathState, completion: @escaping @MainActor (String) -> Void) {
        presentScanner(navigation: navigation, scanType: .universal, completion: completion)
    }

    func scanRecipient(navigation: NavigationPathState, completion: @escaping @MainActor (String) -> Void) {
        presentScanner(navigation: navigation, scanType: .address, completion: completion)
    }

    private func presentScanner(navigation: NavigationPathState, scanType: QRScanType, completion: @escaping @MainActor (String) -> Void) {
        guard let featureModel else { return }
        let expectedRevision = revision
        let signerWalletId = selectedSignerWalletId
        scanner.present(navigation: navigation, isActive: { [weak self, weak featureModel] in
            guard let self, let featureModel else { return false }
            return isActive && revision == expectedRevision && selectedSignerWalletId == signerWalletId && self.featureModel === featureModel
        }, scanType: scanType, onCode: completion)
    }

    func presentDedicatedCreation() {
        closeIdentityModal()
        let ticket = UUID()
        let model = factory.createWalletScene(onComplete: nil, purpose: .dedicatedMultisigIdentity { [weak self] wallet in
            self?.completeIdentity(wallet, ticket: ticket, completion: .created)
        })
        identityModal = IdentityModal(id: ticket, content: .create(model))
    }

    func presentRecovery(dedicated: Bool) {
        closeIdentityModal()
        let ticket = UUID()
        let purpose: WalletOnboardingPurpose = dedicated ? .dedicatedMultisigIdentity { [weak self] wallet in
            self?.completeIdentity(wallet, ticket: ticket, completion: .recovered)
        } : .personal
        let model = factory.importWalletScene(onComplete: { [weak self] in
            guard let self, let id = factory.walletSessionService.currentWalletId,
                  let wallet = try? factory.storeManager.walletStore.getWallet(id: id) else { return }
            completeIdentity(wallet, ticket: ticket, completion: .recovered)
        }, purpose: purpose)
        identityModal = IdentityModal(id: ticket, content: .recovery(model))
    }

    func closeIdentityModal() {
        switch identityModal?.content {
        case let .create(model): model.cancel()
        case let .recovery(model): model.cancel()
        case nil: break
        }
        identityModal = nil
    }

    func closeIdentityModal(id: UUID) {
        guard identityModal?.id == id else { return }
        closeIdentityModal()
    }

    func preservesWalletChange(_ walletId: WalletId?) -> Bool {
        guard isActive else { return false }
        if identityModal != nil {
            return true
        }
        guard let walletId, expectedCurrentWalletId == walletId else { return false }
        expectedCurrentWalletId = nil
        return true
    }

    func cancel() {
        authorization.cancel()
        isActive = false
        revision &+= 1
        loadingTask?.cancel()
        loadingTask = nil
        featureModel?.cancel()
        featureModel = nil
        boundWallet = nil
        scanner.cancel()
        closeIdentityModal()
        isSelecting = false
        expectedCurrentWalletId = nil
    }

    private func completeIdentity(_ wallet: Wallet, ticket: UUID, completion: IdentityCompletion) {
        guard identityModal?.id == ticket, isActive else { return }
        expectedCurrentWalletId = factory.walletSessionService.currentWalletId
        closeIdentityModal()
        replaceFeature(with: wallet)
        if case .recovered = completion {
            entry = .list
        }
        onIdentityComplete(wallet, completion)
    }

    private func replaceFeature(with wallet: Wallet) {
        revision &+= 1
        loadingTask?.cancel()
        loadingTask = nil
        featureModel?.cancel()
        featureModel = factory.bsvMultisigScene(wallet: wallet, authorization: authorization, cancelAuthorization: authorization.cancel(scopeId:))
        boundWallet = wallet
        selectedSignerWalletId = wallet.id
        scanner.cancel()
        isSelecting = false
    }

    private func isCurrent(_ expected: UInt) -> Bool {
        isActive && expected == revision && !Task.isCancelled
    }
}

@MainActor
extension BsvMultisigRoute {
    func walletId(in model: BsvMultisigViewModel?) -> String? {
        switch self {
        case let .wallet(id), let .invite(id, _), let .members(id), let .backup(id), let .receive(id), let .send(id), let .configuration(id), let .pending(id), let .history(id), let .transaction(id, _), let .paymail(id), let .settings(id), let .funding(id), let .fundingResult(id): id
        case let .proposal(id), let .review(id, _): model?.proposals.first { $0.id == id }?.walletId
        case .list, .create, .createReview, .join, .joinReview, .fundingReview: nil
        }
    }
}
