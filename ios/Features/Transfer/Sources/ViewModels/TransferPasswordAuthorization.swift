// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeConfirmAuthorization
import protocol BsvHomeCore.BsvHomeOfflineSigningPasswordServiceProtocol
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeCoreError
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.Wallet

@Observable
@MainActor
public final class TransferPasswordAuthorization: BsvHomeConfirmAuthorization {
    public private(set) var presentation: TransferPasswordViewModel?

    private let makeService: (@MainActor (String) throws -> any BsvHomePortalTransferPasswordServiceProtocol)?
    private var continuation: CheckedContinuation<Void, any Error>?
    private(set) var requestId: UUID?

    public init(makeService: @escaping @MainActor (String) throws -> any BsvHomePortalTransferPasswordServiceProtocol) {
        self.makeService = makeService
    }

    private init() {
        makeService = nil
    }

    public static func offline() -> TransferPasswordAuthorization {
        TransferPasswordAuthorization()
    }

    public func authorize(wallet: Wallet, transfer _: BsvHomeTransferData) async throws {
        guard let makeService else { throw BsvHomeCoreError.Cancelled }
        try await authorize(wallet: wallet, requestId: UUID(), check: {}, makeService: {
            try makeService(wallet.id)
        })
    }

    public func authorize(
        wallet: Wallet,
        requestId: UUID,
        check: @escaping @MainActor () throws -> Void,
        makeService: @escaping @MainActor () throws -> any BsvHomePortalTransferPasswordServiceProtocol,
    ) async throws {
        guard self.makeService != nil else { throw BsvHomeCoreError.Cancelled }
        try await authorize(wallet: wallet, requestId: requestId, check: check, backend: .online(makeService))
    }

    public func authorizeOffline(
        wallet: Wallet,
        requestId: UUID,
        check: @escaping @MainActor () throws -> Void,
        makeService: @escaping @MainActor () throws -> any BsvHomeOfflineSigningPasswordServiceProtocol,
    ) async throws {
        try await authorize(wallet: wallet, requestId: requestId, check: check, backend: .offline(makeService))
    }

    private func authorize(
        wallet: Wallet,
        requestId: UUID,
        check: @escaping @MainActor () throws -> Void,
        backend: TransferPasswordBackend,
    ) async throws {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                do {
                    guard !Task.isCancelled, self.continuation == nil else { throw BsvHomeCoreError.Cancelled }
                    try check()
                    self.continuation = continuation
                    self.requestId = requestId
                    presentation = TransferPasswordViewModel(
                        mode: .authorization,
                        walletName: wallet.name,
                        backend: backend.checking { [weak self] in
                            do {
                                guard self?.requestId == requestId else { throw BsvHomeCoreError.Cancelled }
                                try check()
                            } catch {
                                self?.cancel(requestId: requestId)
                                throw error
                            }
                        },
                        onComplete: { [weak self] in self?.complete(requestId: requestId) },
                        onCancel: { [weak self] in self?.cancel(requestId: requestId) },
                    )
                } catch {
                    continuation.resume(throwing: error)
                }
            }
            guard !Task.isCancelled else { throw BsvHomeCoreError.Cancelled }
            try check()
        } onCancel: {
            Task { @MainActor [weak self] in self?.cancel(requestId: requestId) }
        }
    }

    public func cancel() {
        let continuation = continuation
        self.continuation = nil
        requestId = nil
        let presentation = presentation
        self.presentation = nil
        presentation?.cancel()
        continuation?.resume(throwing: BsvHomeCoreError.Cancelled)
    }

    public func cancel(requestId: UUID) {
        guard self.requestId == requestId else { return }
        cancel()
    }

    private func complete(requestId: UUID) {
        guard self.requestId == requestId else { return }
        let continuation = continuation
        self.continuation = nil
        self.requestId = nil
        presentation = nil
        continuation?.resume()
    }
}
