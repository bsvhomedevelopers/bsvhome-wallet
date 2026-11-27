// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import protocol BsvHomeCore.BsvHomeWalletSessionServiceProtocol
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import Primitives
import UIKit

/// OnstartService runs services before the app starts.
/// See OnstartAsyncService for any background tasks to run after start
public struct OnstartService: Sendable {
    private let appStartService: any BsvHomeAppStartServiceProtocol
    private let preferencesService: any BsvHomePreferencesServiceProtocol
    private let keystore: any Keystore
    private let session: any BsvHomeWalletSessionServiceProtocol

    public init(
        appStartService: any BsvHomeAppStartServiceProtocol,
        preferencesService: any BsvHomePreferencesServiceProtocol,
        keystore: any Keystore,
        session: any BsvHomeWalletSessionServiceProtocol,
    ) {
        self.appStartService = appStartService
        self.preferencesService = preferencesService
        self.keystore = keystore
        self.session = session
    }

    @MainActor
    public func configure() {
        validateDeviceSecurity()
        configureURLCache()
        do {
            try Self.configureCurrency(preferencesService: preferencesService)
            try excludeDirectoriesFromBackup()
            _ = try preferencesService.incrementLaunchesCount()
        } catch {
            debugLog("configure error: \(error)")
        }
    }

    public func setupWallets() async {
        do {
            let failures = try await keystore.migrateV3Keystores(for: await session.getWallets())
            for failure in failures {
                debugLog("v3 keystore migration failed for \(failure.walletId.id): \(failure.error)")
            }
        } catch {
            debugLog("v3 keystore migration could not enumerate wallets: \(error)")
        }
        for failure in await appStartService.setupWallets() {
            debugLog("wallet setup \(failure.step) failed: \(failure.message)")
        }
    }
}

// MARK: - Private

extension OnstartService {
    private func configureURLCache() {
        URLCache.shared.memoryCapacity = 256_000_000 // ~256 MB memory space
        URLCache.shared.diskCapacity = 1_000_000_000 // ~1GB disk cache space
    }

    private func excludeDirectoriesFromBackup() throws {
        let excludedBackupDirectories: [FileManager.Directory] = [.documents, .applicationSupport, .library(.preferences)]
        for directory in excludedBackupDirectories {
            try FileManager.default.addSkipBackupAttributeToItemAtURL(directory.url)

            #if DEBUG
                debugLog("Excluded backup directory: \(directory.directory)")
            #endif
        }
    }

    @MainActor
    private func validateDeviceSecurity() {
        let device = UIDevice.current
        if !device.isSimulator, device.isJailBroken || device.isFridaDetected {
            fatalError()
        }
    }

    static func configureCurrency(preferencesService: any BsvHomePreferencesServiceProtocol) throws {
        try preferencesService.setCurrency(currency: preferencesService.getCurrency())
    }
}
