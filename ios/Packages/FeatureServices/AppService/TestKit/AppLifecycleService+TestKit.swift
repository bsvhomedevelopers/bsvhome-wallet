// Copyright (c). Gem Wallet. All rights reserved.

@testable import AppService
import ConnectionStatusService
import Foundation
import class BsvHomeCore.BsvHomeConnectionService
import protocol BsvHomeCore.BsvHomeDeviceServiceProtocol
import protocol BsvHomeCore.BsvHomePerpetualServiceProtocol
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import class BsvHomeCore.BsvHomeWalletSessionService
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Store
import StoreTestKit
import StreamService
import StreamServiceTestKit
import WalletConnectorService
import WalletConnectorServiceTestKit

public extension AppLifecycleService {
    static func mock(
        walletConnector: any WalletConnectorServiceable = WalletConnectorServiceMock(),
        connectionStatusObserver: ConnectionStatusObserver = ConnectionStatusObserver(connectionService: BsvHomeConnectionService(), monitors: []),
        deviceService: any BsvHomeDeviceServiceProtocol = BsvHomeDeviceServiceMock(),
        subscriptionsObserver: SubscriptionsObserver = .mock(),
        streamObserverService: StreamObserverService = .mock(),
        hyperliquidObserverService: PerpetualObserverMock = PerpetualObserverMock(),
        perpetualService: any BsvHomePerpetualServiceProtocol = BsvHomePerpetualServiceMock(),
        walletSessionService: any BsvHomeWalletSessionServiceProtocol = BsvHomeWalletSessionService.mock(),
        transactionStateService: any BsvHomeTransactionStateServiceProtocol = BsvHomeTransactionStateServiceMock(),
        bsvReceiptObserver: BsvReceiptObserverService? = nil,
        environment: BsvHomeRuntimeEnvironment = .production,
    ) -> AppLifecycleService {
        AppLifecycleService(
            walletConnector: walletConnector,
            connectionStatusObserver: connectionStatusObserver,
            deviceService: deviceService,
            subscriptionsObserver: subscriptionsObserver,
            streamObserverService: streamObserverService,
            perpetualService: perpetualService,
            perpetualObserver: hyperliquidObserverService,
            walletSessionService: walletSessionService,
            transactionStateService: transactionStateService,
            bsvReceiptObserver: bsvReceiptObserver,
            environment: environment,
        )
    }
}
