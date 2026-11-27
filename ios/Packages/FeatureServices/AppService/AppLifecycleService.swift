// Copyright (c). Gem Wallet. All rights reserved.

import ConnectionStatusService
import Foundation
import protocol BsvHomeCore.BsvHomeDeviceServiceProtocol
import protocol BsvHomeCore.BsvHomePerpetualServiceProtocol
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Primitives
import Store
import StreamService
import SwiftUI
import WalletConnectorService

public actor AppLifecycleService: Sendable {
    private let walletConnector: any WalletConnectorServiceable
    private let connectionStatusObserver: ConnectionStatusObserver
    private let deviceService: any BsvHomeDeviceServiceProtocol
    private let subscriptionsObserver: SubscriptionsObserver
    private let streamObserverService: StreamObserverService
    private let perpetualService: any BsvHomePerpetualServiceProtocol
    private let perpetualObserver: any PerpetualObservable
    private let walletSessionService: any BsvHomeWalletSessionServiceProtocol
    private let transactionStateService: any BsvHomeTransactionStateServiceProtocol
    private let bsvReceiptObserver: BsvReceiptObserverService?
    private let environment: BsvHomeRuntimeEnvironment

    public init(
        walletConnector: any WalletConnectorServiceable,
        connectionStatusObserver: ConnectionStatusObserver,
        deviceService: any BsvHomeDeviceServiceProtocol,
        subscriptionsObserver: SubscriptionsObserver,
        streamObserverService: StreamObserverService,
        perpetualService: any BsvHomePerpetualServiceProtocol,
        perpetualObserver: any PerpetualObservable,
        walletSessionService: any BsvHomeWalletSessionServiceProtocol,
        transactionStateService: any BsvHomeTransactionStateServiceProtocol,
        bsvReceiptObserver: BsvReceiptObserverService? = nil,
        environment: BsvHomeRuntimeEnvironment = .production,
    ) {
        self.walletConnector = walletConnector
        self.connectionStatusObserver = connectionStatusObserver
        self.deviceService = deviceService
        self.subscriptionsObserver = subscriptionsObserver
        self.streamObserverService = streamObserverService
        self.perpetualService = perpetualService
        self.perpetualObserver = perpetualObserver
        self.walletSessionService = walletSessionService
        self.transactionStateService = transactionStateService
        self.bsvReceiptObserver = bsvReceiptObserver
        self.environment = environment
    }

    public func setup() async {
        async let walletConnect: () = setupWalletConnect()
        async let device: () = setupDeviceObserver()
        async let observers: () = connectObservers()

        _ = await (walletConnect, device, observers)
    }

    public func updateWalletConnections() async {
        if environment.usesBsvReceiptEvents() {
            await bsvReceiptObserver?.update()
        }
        async let perpetual: () = connectPerpetual()
        async let stream: () = streamObserverService.update()
        _ = await (perpetual, stream)
    }

    public func updatePerpetualConnection() async {
        let wallet = await walletSessionService.currentWallet
        do {
            let connect = try await perpetualService.syncEnablement(wallet: wallet?.map(), trigger: .scheduled)
            await updatePerpetualObserver(wallet: wallet, connect: connect)
        } catch {
            debugLog("AppLifecycleService perpetual enablement error: \(error)")
        }
    }

    public func handleScenePhase(_ phase: ScenePhase) async {
        switch phase {
        case .active:
            debugLog("AppLifecycleService: App active — connecting observers")
            await connectObservers()
        case .background:
            debugLog("AppLifecycleService: App background — disconnecting observers")
            await disconnectObservers()
        case .inactive:
            debugLog("AppLifecycleService: App inactive")
        @unknown default:
            break
        }
    }
}

// MARK: - Private

extension AppLifecycleService {
    private func setupWalletConnect() async {
        guard environment.usesBsvHomeHostedServices() else { return }
        do {
            try walletConnector.configure()
            if try await walletConnector.hasSessions() {
                await walletConnector.setup()
            }
        } catch {
            debugLog("AppLifecycleService setupWalletConnect error: \(error)")
        }
    }

    private func setupDeviceObserver() async {
        guard environment.usesBsvHomeHostedServices() else { return }
        await synchronizeSubscriptions(subscriptionsObserver.observe().dropFirst())
    }

    func synchronizeSubscriptions(_ subscriptions: some AsyncSequence) async {
        do {
            for try await _ in subscriptions {
                guard !Task.isCancelled else { return }
                do {
                    try await deviceService.synchronizeIfNeeded()
                } catch {
                    guard !Task.isCancelled else { return }
                    debugLog("AppLifecycleService subscription sync error: \(error)")
                }
            }
        } catch {
            debugLog("AppLifecycleService setupDeviceObserver error: \(error)")
        }
    }

    private func connectObservers() async {
        transactionStateService.resumeTracking()
        if environment.usesBsvReceiptEvents() {
            await bsvReceiptObserver?.connect()
        }
        async let connection: () = connectionStatusObserver.start()
        async let stream: () = streamObserverService.connect()
        async let perpetual: () = connectPerpetual()
        async let pending: () = trackPendingTransactions()
        _ = await (connection, stream, perpetual, pending)
    }

    private func trackPendingTransactions() async {
        do {
            try await transactionStateService.trackPending()
        } catch {
            debugLog("AppLifecycleService pending tracking error: \(error)")
        }
    }

    private func connectPerpetual() async {
        let wallet = await walletSessionService.currentWallet
        let connect = perpetualService.shouldConnectPerpetuals(wallet: wallet?.map())
        await updatePerpetualObserver(wallet: wallet, connect: connect)
    }

    private func updatePerpetualObserver(wallet: Wallet?, connect: Bool) async {
        if connect, let wallet {
            await perpetualObserver.setup(for: wallet)
        } else {
            await perpetualObserver.disconnect()
        }
    }

    private func disconnectObservers() async {
        transactionStateService.pauseTracking()
        async let connection: () = connectionStatusObserver.stop()
        async let price: () = streamObserverService.disconnect()
        async let perpetual: () = perpetualObserver.disconnect()
        async let receipts: Void? = bsvReceiptObserver?.disconnect()
        _ = await (connection, price, perpetual, receipts)
    }
}
