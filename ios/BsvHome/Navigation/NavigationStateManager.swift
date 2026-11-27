// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import Primitives
import SwiftUI

@Observable
final class NavigationStateManager: Sendable {
    private struct PresentationPersistence: @unchecked Sendable {
        let defaults: UserDefaults
        let key: String?

        init(defaults: UserDefaults, portalOrigin: String) {
            self.defaults = defaults
            key = portalOrigin.isEmpty ? nil : "WowPayWalletPresentation.v1." + portalOrigin
        }

        func restore() -> WalletPresentation {
            guard let key else { return .local }
            guard let data = defaults.data(forKey: key),
                  let decoded = try? JSONDecoder().decode(WalletPresentation.self, from: data),
                  let selection = decoded.persistentSelection
            else {
                defaults.removeObject(forKey: key)
                return .local
            }
            return selection
        }

        func save(_ presentation: WalletPresentation) {
            guard let key else { return }
            guard let selection = presentation.persistentSelection,
                  let data = try? JSONEncoder().encode(selection)
            else {
                defaults.removeObject(forKey: key)
                return
            }
            defaults.set(data, forKey: key)
        }
    }

    private let presentationPersistence: PresentationPersistence
    private let initialPresentation: WalletPresentation

    @MainActor
    private var selectedWalletPresentation: WalletPresentation?

    @MainActor
    var home = NavigationPathState()
    @MainActor
    var swap = NavigationPathState()
    @MainActor
    var wallet = NavigationPathState()
    @MainActor
    var settings = NavigationPathState()

    @ObservationIgnored
    @MainActor
    private var tabExitObservers: [UUID: (tab: TabItem, onExit: @MainActor () -> Void)] = [:]

    @ObservationIgnored
    @MainActor
    private var tabExitOwner: (id: UUID, onExit: @MainActor () -> Void)?

    @MainActor
    var selectedTab: TabItem = .home {
        willSet {
            guard selectedTab != newValue else { return }
            let onExit = tabExitOwner?.onExit
            let callbacks = tabExitObservers.filter { $0.value.tab == selectedTab }
            for id in callbacks.keys {
                tabExitObservers[id] = nil
            }
            onExit?()
            for observer in callbacks.values {
                observer.onExit()
            }
        }
    }

    @MainActor
    var walletPresentation: WalletPresentation {
        get { selectedWalletPresentation ?? initialPresentation }
        set {
            selectedWalletPresentation = newValue
            presentationPersistence.save(newValue)
        }
    }

    @MainActor
    var previousSelectedTab: TabItem = .home

    @MainActor
    var walletTabReselected = false

    @MainActor
    var detailSwapDestination: PortalSwapDestination?
    @MainActor
    var detailSwapDepth = 0

    @MainActor
    var pendingWalletPath: [any Hashable & Codable] = []
    @MainActor
    var onSettingsReset: (@MainActor () -> Void)?

    init(defaults: UserDefaults = .standard, portalOrigin: String = Bundle.main.object(forInfoDictionaryKey: "WowPayPortalOrigin") as? String ?? "") {
        let persistence = PresentationPersistence(defaults: defaults, portalOrigin: portalOrigin)
        presentationPersistence = persistence
        initialPresentation = persistence.restore()
    }
}

// MARK: - Business Logic

@MainActor
extension NavigationStateManager {
    @discardableResult
    func registerTabExitOwner(onExit: @escaping @MainActor () -> Void) -> UUID {
        let id = UUID()
        tabExitOwner = (id, onExit)
        return id
    }

    func removeTabExitOwner(_ id: UUID) {
        guard tabExitOwner?.id == id else { return }
        tabExitOwner = nil
    }

    @discardableResult
    func observeTabExit(tab: TabItem, onExit: @escaping @MainActor () -> Void) -> UUID {
        let id = UUID()
        tabExitObservers[id] = (tab, onExit)
        return id
    }

    func removeTabExitObserver(_ id: UUID) {
        tabExitObservers[id] = nil
    }

    func select(tab: TabItem) {
        selectedTab = tab
        // back to root if selected same tab, if some routes already in stack
        guard tab != previousSelectedTab else {
            backToRoot(tab: tab)
            return
        }
        previousSelectedTab = selectedTab
    }

    func backToRoot(tab: TabItem) {
        if tab == .wallet, wallet.isEmpty {
            walletTabReselected.toggle()
        }

        switch tab {
        case .wallet:
            detailSwapDestination = nil
            wallet.reset()
        case .home: home.reset()
        case .swap: swap.reset()
        case .settings:
            onSettingsReset?()
            settings.reset()
        }
    }

    func reset() {
        detailSwapDestination = nil
        for tabItem in TabItem.allCases {
            backToRoot(tab: tabItem)
        }
        selectedTab = pendingWalletPath.isEmpty ? .home : .wallet
        previousSelectedTab = selectedTab
        wallet.setPath(pendingWalletPath)
        pendingWalletPath = []
    }

    func openAsset(_ asset: Asset) {
        guard allows(assetId: asset.id) else { return }
        selectWalletPresentation(.local)
        if asset.type == .perpetual {
            wallet.append(Scenes.Perpetual(asset))
        } else {
            wallet.append(Scenes.Asset(asset: asset))
        }
        selectedTab = .wallet
        previousSelectedTab = .wallet
    }

    func openSwap(assetId: AssetId) {
        guard allows(assetId: assetId) else { return }
        let destination = PortalSwapDestination(entryAsset: assetId, id: UUID())
        detailSwapDestination = destination
        wallet.append(destination)
        detailSwapDepth = wallet.count
        selectedTab = .wallet
        previousSelectedTab = .wallet
    }

    func openMultisig() {
        let asset = Chain.bitcoinSv.asset
        guard allows(assetId: asset.id) else { return }
        wallet.append(Scenes.Asset(asset: asset, initialTab: .multisig))
        selectedTab = .wallet
        previousSelectedTab = .wallet
    }

    func selectWalletPresentation(_ presentation: WalletPresentation, preservingNavigation: NavigationPathState? = nil) {
        let selection = presentation.persistentSelection ?? .local
        let previous = walletPresentation
        walletPresentation = selection
        guard previous != selection else { return }
        if preservingNavigation !== settings {
            onSettingsReset?()
        }
        detailSwapDestination = nil
        detailSwapDepth = 0
        pendingWalletPath = []
        if preservingNavigation !== home {
            home.reset()
        }
        if preservingNavigation !== swap {
            swap.reset()
        }
        if preservingNavigation !== wallet {
            wallet.reset()
        }
        if preservingNavigation !== settings {
            settings.reset()
        }
    }

    private func allows(assetId: AssetId) -> Bool {
        BsvHomeRuntimeEnvironment.current.allowedAssetIds()?.contains(assetId.identifier) ?? true
    }

    func reconcileDetailSwapPath() {
        if detailSwapDestination != nil, wallet.count < detailSwapDepth {
            detailSwapDestination = nil
        }
    }

    func openWallet(path: [any Hashable & Codable]) {
        detailSwapDestination = nil
        wallet.setPath(path)
        selectedTab = .wallet
        previousSelectedTab = .wallet
    }
}
