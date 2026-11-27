// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot

public final class BsvHomePortalTransferPasswordServiceMock: BsvHomePortalTransferPasswordServiceProtocol, @unchecked Sendable {
    private let snapshot: BsvHomeTransferPasswordSnapshot
    private let cancellationLock = NSLock()
    private var cancellationCount = 0
    @MainActor public var onStatus: (@MainActor () async throws -> BsvHomeTransferPasswordSnapshot)?
    @MainActor public var onSet: (@MainActor (BsvHomeTransferPasswordSnapshot, String?, String, String) async throws -> BsvHomeTransferPasswordSnapshot)?
    @MainActor public var onVerify: (@MainActor (BsvHomeTransferPasswordSnapshot, String) async throws -> Void)?
    @MainActor public private(set) var statusCount = 0
    @MainActor public private(set) var setCount = 0
    @MainActor public private(set) var verifyCount = 0

    public init(snapshot: BsvHomeTransferPasswordSnapshot = BsvHomeTransferPasswordSnapshot(
        walletId: "test-wallet",
        address: "1BitcoinEaterAddressDontSendf59kuE",
        configured: true,
        generation: 0,
    )) {
        self.snapshot = snapshot
    }

    public var cancelCount: Int { cancellationLock.withLock { cancellationCount } }

    @MainActor
    public func status() async throws -> BsvHomeTransferPasswordSnapshot {
        statusCount += 1
        return try await onStatus?() ?? snapshot
    }

    @MainActor
    public func set(snapshot: BsvHomeTransferPasswordSnapshot, currentPassword: String?, newPassword: String, confirmation: String) async throws -> BsvHomeTransferPasswordSnapshot {
        setCount += 1
        if let onSet {
            return try await onSet(snapshot, currentPassword, newPassword, confirmation)
        }
        return BsvHomeTransferPasswordSnapshot(walletId: snapshot.walletId, address: snapshot.address, configured: true, generation: snapshot.generation)
    }

    @MainActor
    public func verify(snapshot: BsvHomeTransferPasswordSnapshot, password: String) async throws {
        verifyCount += 1
        try await onVerify?(snapshot, password)
    }

    public func cancel() {
        cancellationLock.withLock { cancellationCount += 1 }
    }
}
