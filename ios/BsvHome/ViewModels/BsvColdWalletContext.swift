import BsvColdWallet
import Foundation
import SwiftUI

@Observable
@MainActor
final class BsvColdWalletContext: Identifiable {
    let id: UUID
    let model: BsvColdWalletViewModel
    let authorization: BsvColdWalletPasswordAuthorization
    let onboardingOwner: ObjectIdentifier?
    @ObservationIgnored private weak var tabExitNavigation: NavigationStateManager?
    @ObservationIgnored private var tabExitObserver: UUID?
    @ObservationIgnored private var walletSelectionObserver: (@MainActor (String) -> Void)?
    @ObservationIgnored private var claimedWalletId: String?

    var recipientScan: RecipientScan?
    var onScanRecipient: BsvColdWalletRecipientScanner?
    var onInitialPageBack: (@MainActor () -> Void)?

    struct RecipientScan: Identifiable {
        let id = UUID()
        let contextId: UUID
        let path: NavigationPath
        let completion: @MainActor (String) -> Void
    }

    init(id: UUID = UUID(), model: BsvColdWalletViewModel, authorization: BsvColdWalletPasswordAuthorization, onboardingOwner: ObjectIdentifier? = nil) {
        self.id = id
        self.model = model
        self.authorization = authorization
        self.onboardingOwner = onboardingOwner
    }

    deinit {
        let navigation = tabExitNavigation
        let observer = tabExitObserver
        if let observer {
            Task { @MainActor in navigation?.removeTabExitObserver(observer) }
        }
    }

    func observeTabExit(in navigation: NavigationStateManager, tab: TabItem) {
        removeTabExitObserver()
        guard !model.isCancelled, navigation.selectedTab == tab else { cancel(); return }
        tabExitNavigation = navigation
        tabExitObserver = navigation.observeTabExit(tab: tab) { [weak self] in self?.cancel() }
    }

    private func removeTabExitObserver() {
        if let tabExitObserver {
            tabExitNavigation?.removeTabExitObserver(tabExitObserver)
        }
        tabExitObserver = nil
        tabExitNavigation = nil
    }

    func observeWalletSelection(_ observer: @escaping @MainActor (String) -> Void) {
        guard !model.isCancelled else { return }
        walletSelectionObserver = observer
        if let claimedWalletId {
            observer(claimedWalletId)
        }
    }

    func consumeWalletSelection(walletId: String) throws -> Bool {
        guard !model.isCancelled, try model.consumeWalletSelection(walletId: walletId) else { return false }
        claimedWalletId = walletId
        walletSelectionObserver?(walletId)
        return true
    }

    func cancelActions() {
        recipientScan = nil
        onScanRecipient = nil
        onInitialPageBack = nil
        model.cancelActions()
        authorization.cancel()
    }

    func cancelForNavigationRemoval() {
        recipientScan = nil
        onScanRecipient = nil
        onInitialPageBack = nil
        model.cancelForNavigationRemoval()
        cancel()
    }

    func cancel() {
        recipientScan = nil
        onScanRecipient = nil
        onInitialPageBack = nil
        walletSelectionObserver = nil
        removeTabExitObserver()
        model.cancel()
        authorization.cancel()
    }
}
