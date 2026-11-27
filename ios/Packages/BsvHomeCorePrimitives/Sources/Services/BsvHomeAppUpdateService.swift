// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeAppUpdateServiceProtocol
import Primitives

public extension BsvHomeAppUpdateServiceProtocol {
    func newestRelease() async -> Release? {
        try? await newest(store: PlatformStore.current.map(), currentVersion: Bundle.main.releaseVersionNumber).map { $0.map() }
    }

    func checkForUpdate() async throws -> Release? {
        try await check(store: PlatformStore.current.map(), currentVersion: Bundle.main.releaseVersionNumber).map { $0.map() }
    }
}
