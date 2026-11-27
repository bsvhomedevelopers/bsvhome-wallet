// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeOfflineSigningPasswordServiceProtocol
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot

public final class BsvHomeOfflineSigningPasswordServiceMock: BsvHomeOfflineSigningPasswordServiceProtocol, @unchecked Sendable {
    private let lock = NSLock()
    private let snapshot: BsvHomeTransferPasswordSnapshot
    private let onStatus: (@Sendable () async throws -> BsvHomeTransferPasswordSnapshot)?
    private let onSet: (@Sendable (BsvHomeTransferPasswordSnapshot, String?, String, String) async throws -> BsvHomeTransferPasswordSnapshot)?
    private let onVerify: (@Sendable (BsvHomeTransferPasswordSnapshot, String) async throws -> Void)?
    private let onCancel: (@Sendable () -> Void)?
    private var statusCalls = 0
    private var setCalls = 0
    private var verifyCalls = 0
    private var cancelCalls = 0
    private var observedTask: Task<Void, Never>?
    private var cancelledTaskObservations: [Bool] = []

    public init(
        snapshot: BsvHomeTransferPasswordSnapshot = BsvHomeTransferPasswordSnapshot(walletId: "test-wallet", address: "1BitcoinEaterAddressDontSendf59kuE", configured: true, generation: 0),
        onStatus: (@Sendable () async throws -> BsvHomeTransferPasswordSnapshot)? = nil,
        onSet: (@Sendable (BsvHomeTransferPasswordSnapshot, String?, String, String) async throws -> BsvHomeTransferPasswordSnapshot)? = nil,
        onVerify: (@Sendable (BsvHomeTransferPasswordSnapshot, String) async throws -> Void)? = nil,
        onCancel: (@Sendable () -> Void)? = nil,
    ) {
        self.snapshot = snapshot
        self.onStatus = onStatus
        self.onSet = onSet
        self.onVerify = onVerify
        self.onCancel = onCancel
    }

    public var statusCount: Int { lock.withLock { statusCalls } }
    public var setCount: Int { lock.withLock { setCalls } }
    public var verifyCount: Int { lock.withLock { verifyCalls } }
    public var cancelCount: Int { lock.withLock { cancelCalls } }
    public var taskCancellationAtCancel: [Bool] { lock.withLock { cancelledTaskObservations } }

    public func observeCancellation(of task: Task<Void, Never>) {
        lock.withLock { observedTask = task }
    }

    public func status() async throws -> BsvHomeTransferPasswordSnapshot {
        lock.withLock { statusCalls += 1 }
        return try await onStatus?() ?? snapshot
    }

    public func set(snapshot: BsvHomeTransferPasswordSnapshot, currentPassword: String?, newPassword: String, confirmation: String) async throws -> BsvHomeTransferPasswordSnapshot {
        lock.withLock { setCalls += 1 }
        if let onSet {
            return try await onSet(snapshot, currentPassword, newPassword, confirmation)
        }
        return BsvHomeTransferPasswordSnapshot(walletId: snapshot.walletId, address: snapshot.address, configured: true, generation: snapshot.generation)
    }

    public func verify(snapshot: BsvHomeTransferPasswordSnapshot, password: String) async throws {
        lock.withLock { verifyCalls += 1 }
        try await onVerify?(snapshot, password)
    }

    public func cancel() {
        lock.withLock {
            cancelCalls += 1
            if let observedTask {
                cancelledTaskObservations.append(observedTask.isCancelled)
            }
        }
        onCancel?()
    }
}
