// Copyright (c). Gem Wallet. All rights reserved.

import AppService
import Components
import BsvHomeCoreServices
import Localization
import LockManager
import Primitives
import Store
import Style
import SwiftUI
import WidgetKit

@main
struct BsvHomeApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        WindowGroup {
            switch appDelegate.launchState {
            case .launching:
                Color.clear
            case .choosePurpose:
                DevicePurposeScene(confirm: appDelegate.confirmDevicePurpose)
            case .configured(.coldWallet):
                ColdWalletRootScene()
            case .configured(.daily):
                dailyRoot(resolver: .main)
            case .purposeUnavailable:
                ContentUnavailableView {
                    Label(Localized.devicePurposeTitle, systemImage: "exclamationmark.triangle")
                } description: {
                    Text(Localized.devicePurposeStorageError)
                } actions: {
                    Button(Localized.Common.tryAgain, action: appDelegate.loadDevicePurpose)
                }
            }
        }
    }

    private func dailyRoot(resolver: AppResolver) -> some View {
        RootScene(
            model: RootSceneViewModel(
                observablePreferences: resolver.services.observablePreferences,
                walletConnectorPresenter: resolver.services.walletConnectorPresenter,
                onstartService: resolver.services.onstartService,
                appStartService: resolver.services.appStartService,
                pushNotificationEnablerService: resolver.services.pushNotificationEnablerService,
                appLifecycleService: resolver.services.appLifecycleService,
                navigationHandler: resolver.services.navigationHandler,
                lockWindowManager: LockWindowManager(lockModel: resolver.services.viewModelFactory.lockScene()),
                viewModelFactory: resolver.services.viewModelFactory,
                walletSessionService: resolver.services.walletSessionService,
                appUpdateService: resolver.services.appUpdateService,
                rateService: resolver.services.rateService,
                toastPresenter: resolver.services.toastPresenter,
                deviceService: resolver.services.deviceService,
            ),
        )
        .databaseQueue(resolver.storages.db.dbQueue)
        .navigationBarTitleDisplayMode(.inline)
        .tint(Colors.black)
    }
}

@Observable
class AppDelegate: NSObject, UIApplicationDelegate, UIWindowSceneDelegate {
    enum LaunchState: Equatable {
        case launching
        case choosePurpose
        case configured(DevicePurpose)
        case purposeUnavailable
    }

    private(set) var launchState = LaunchState.launching

    func application(_: UIApplication, supportedInterfaceOrientationsFor _: UIWindow?) -> UIInterfaceOrientationMask {
        OrientationLock.mask
    }

    func application(_: UIApplication, didFinishLaunchingWithOptions _: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        guard !BsvHomeAppHostMode.isUnitTestHost else { return true }
        loadDevicePurpose()
        return true
    }

    func loadDevicePurpose() {
        if case .configured = launchState {
            return
        }
        do {
            let store = DevicePurposeStore.appGroup
            if let purpose = try store.read() {
                openDevicePurpose(purpose)
            } else {
                let purpose = try store.confirm(.daily)
                openDevicePurpose(purpose)
                if purpose == .daily {
                    WidgetCenter.shared.reloadAllTimelines()
                }
            }
        } catch {
            launchState = .purposeUnavailable
        }
    }

    func confirmDevicePurpose(_ purpose: DevicePurpose) throws {
        guard launchState == .choosePurpose else { return }
        let confirmed = try DevicePurposeStore.appGroup.confirm(purpose)
        openDevicePurpose(confirmed)
        if confirmed == .daily {
            WidgetCenter.shared.reloadAllTimelines()
        }
    }

    private func openDevicePurpose(_ purpose: DevicePurpose) {
        launchState = .configured(purpose)
        guard purpose == .daily else { return }

        UNUserNotificationCenter.current().delegate = self
        let services = AppResolver.main.services
        services.onstartService.configure()
        Task {
            for failure in await services.appStartService.run() {
                debugLog("app start \(failure.step) failed: \(failure.message)")
            }
        }
    }

    func application(_: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        guard launchState == .configured(.daily) else { return }
        let token = deviceToken.map { String(format: "%02.2hhx", $0) }.joined()

        Task {
            let _ = try SecurePreferences.standard.set(value: token, key: .deviceToken)
            try await AppResolver.main.services.deviceService.synchronizeIfNeeded()
        }
    }

    func application(_: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: any Error) {
        guard launchState == .configured(.daily) else { return }
        debugLog("didFailToRegisterForRemoteNotificationsWithError error: \(error)")
    }

    func application(_: UIApplication, didReceiveRemoteNotification userInfo: [AnyHashable: Any]) {
        guard launchState == .configured(.daily) else { return }
        Task { await AppResolver.main.services.navigationHandler.handlePush(userInfo) }
    }

    func application(_: UIApplication, open url: URL, options _: [UIApplication.OpenURLOptionsKey: Any] = [:]) -> Bool {
        guard launchState == .configured(.daily) else { return false }
        debugLog("url \(url)")
        return true
    }

    func scene(_: UIScene, openURLContexts _: Set<UIOpenURLContext>) {}

    func scene(_: UIScene, willConnectTo _: UISceneSession, options _: UIScene.ConnectionOptions) {}

    func application(_: UIApplication, shouldAllowExtensionPointIdentifier extensionPointIdentifier: UIApplication.ExtensionPointIdentifier) -> Bool {
        switch extensionPointIdentifier {
        case .keyboard: false
        default: true
        }
    }
}

extension AppDelegate: @preconcurrency UNUserNotificationCenterDelegate {
    func userNotificationCenter(_: UNUserNotificationCenter, willPresent _: UNNotification, withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler(launchState == .configured(.daily) ? [.badge, .banner, .list, .sound] : [])
    }

    func userNotificationCenter(
        _: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void,
    ) {
        if launchState == .configured(.daily) {
            Task { await AppResolver.main.services.navigationHandler.handlePush(response.notification.request.content.userInfo) }
        }
        completionHandler()
    }
}
