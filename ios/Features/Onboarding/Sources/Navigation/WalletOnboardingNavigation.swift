// Copyright (c). Gem Wallet. All rights reserved.

import Components
import SwiftUI

@Observable
@MainActor
final class WalletOnboardingNavigation {
    enum Destination: String, Hashable, Codable {
        case multisig
        case coldCreate
        case coldRecover
        case coldConnect
    }

    let navigation = NavigationPathState()
    private let onMultisigDismiss: (@MainActor () -> Void)?
    private let onColdWalletDismiss: (@MainActor () -> Void)?
    private var isCancelled = false
    @ObservationIgnored private var removalObserver: UUID?
    @ObservationIgnored private var activeEntry: Destination?

    init(onMultisigDismiss: (@MainActor () -> Void)? = nil, onColdWalletDismiss: (@MainActor () -> Void)? = nil) {
        self.onMultisigDismiss = onMultisigDismiss
        self.onColdWalletDismiss = onColdWalletDismiss
    }

    var path: NavigationPath { navigation.path }

    var binding: Binding<NavigationPath> {
        navigation.binding
    }

    func append(_ value: some Hashable) {
        guard !isCancelled else { return }
        var path = navigation.path
        path.append(value)
        navigation.binding.wrappedValue = path
    }

    func appendDistinct(_ value: some Hashable & Codable) {
        guard !isCancelled else { return }
        navigation.append(value)
    }

    func openMultisig() {
        open(.multisig)
    }

    func openColdCreate() {
        open(.coldCreate)
    }

    func openColdRecover() {
        open(.coldRecover)
    }

    func openColdConnect() {
        open(.coldConnect)
    }

    func cancel() {
        guard !isCancelled else { return }
        isCancelled = true
        let entry = activeEntry
        activeEntry = nil
        if let removalObserver {
            navigation.removeRemovalObserver(removalObserver)
        }
        removalObserver = nil
        if let entry {
            dismissed(entry)
        }
    }

    private func open(_ entry: Destination) {
        guard !isCancelled, removalObserver == nil else { return }
        var path = navigation.path
        path.append(entry)
        navigation.binding.wrappedValue = path
        guard !isCancelled else { return }
        activeEntry = entry
        removalObserver = navigation.observeRemoval { [weak self] in
            guard let self else { return }
            activeEntry = nil
            removalObserver = nil
            dismissed(entry)
        }
    }

    private func dismissed(_ entry: Destination) {
        switch entry {
        case .multisig: onMultisigDismiss?()
        case .coldCreate, .coldRecover, .coldConnect: onColdWalletDismiss?()
        }
    }
}
