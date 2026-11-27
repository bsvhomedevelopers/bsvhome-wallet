// Copyright (c). Gem Wallet. All rights reserved.

import protocol BsvHomeCore.BsvHomeTransactionStateServiceProtocol
import protocol BsvHomeCore.BsvHomeAppUpdateServiceProtocol
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import BsvHomeCoreServices
import AppService
import WalletConnectorService
import ConnectionStatusService
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import Foundation
import protocol BsvHomeCore.BsvHomeDeviceServiceProtocol
import Primitives
import PrimitivesComponents
import StreamService
import WalletConnector

extension AppResolver {
    struct Services {
        // Environment-level services
        let walletConnector: WalletConnectorService
        let connectionStatusObserver: ConnectionStatusObserver
        let deviceService: any BsvHomeDeviceServiceProtocol
        let navigationHandler: NavigationHandler
        let navigationPresenter: NavigationPresenter
        let streamObserverService: StreamObserverService
        let transactionStateService: any BsvHomeTransactionStateServiceProtocol
        let observablePreferences: ObservablePreferences
        let walletSessionService: any BsvHomeWalletSessionServiceProtocol
        let appUpdateService: any BsvHomeAppUpdateServiceProtocol
        let rateService: RateService
        let onstartService: OnstartService
        let appStartService: any BsvHomeAppStartServiceProtocol
        let pushNotificationEnablerService: PushNotificationEnablerService
        let walletConnectorPresenter: WalletConnectorPresenter
        let toastPresenter: ToastPresenter
        let viewModelFactory: ViewModelFactory
        let appLifecycleService: AppLifecycleService
    }
}
