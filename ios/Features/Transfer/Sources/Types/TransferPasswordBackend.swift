// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeOfflineSigningPasswordServiceProtocol
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeTransferPasswordError
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot
import Localization
import PrimitivesComponents

public enum TransferPasswordBackend {
    case online(@MainActor () throws -> any BsvHomePortalTransferPasswordServiceProtocol)
    case offline(@MainActor () throws -> any BsvHomeOfflineSigningPasswordServiceProtocol)

    @MainActor
    var title: String {
        switch self {
        case .online: Localized.wowpayTransferPasswordTitle
        case .offline: Localized.wowpayOfflineSigningPasswordTitle
        }
    }

    @MainActor
    func makeService() throws -> Service {
        switch self {
        case let .online(factory): try .online(factory())
        case let .offline(factory): try .offline(factory())
        }
    }

    @MainActor
    func checking(_ check: @escaping @MainActor () throws -> Void) -> Self {
        switch self {
        case let .online(factory): .online { try check(); return try factory() }
        case let .offline(factory): .offline { try check(); return try factory() }
        }
    }

    @MainActor
    func errorDescription(_ error: any Error) -> String {
        let error = error as? BsvHomeTransferPasswordError ?? .Unavailable
        switch self {
        case .online: return error.localizedDescription
        case .offline:
            switch error {
            case .CurrentPasswordRequired, .IncorrectPassword: return Localized.wowpayOfflineSigningPasswordIncorrect
            case .RateLimited: return Localized.wowpayOfflineSigningPasswordRateLimited
            case .Unavailable, .InvalidConfiguration, .InvalidResponse, .NotConfigured: return Localized.wowpayOfflineSigningPasswordUnavailable
            case .InvalidSnapshot, .WatchOnly, .MissingBsvAccount, .WalletChanged, .Cancelled, .InvalidPassword, .ConfirmationMismatch:
                return error.localizedDescription
            }
        }
    }

    enum Service: Sendable {
        case online(any BsvHomePortalTransferPasswordServiceProtocol)
        case offline(any BsvHomeOfflineSigningPasswordServiceProtocol)

        func status() async throws -> BsvHomeTransferPasswordSnapshot {
            switch self {
            case let .online(service): try await service.status()
            case let .offline(service): try await offMain { try await service.status() }
            }
        }

        func set(snapshot: BsvHomeTransferPasswordSnapshot, currentPassword: String?, newPassword: String, confirmation: String) async throws -> BsvHomeTransferPasswordSnapshot {
            switch self {
            case let .online(service):
                try await service.set(snapshot: snapshot, currentPassword: currentPassword, newPassword: newPassword, confirmation: confirmation)
            case let .offline(service):
                try await offMain { try await service.set(snapshot: snapshot, currentPassword: currentPassword, newPassword: newPassword, confirmation: confirmation) }
            }
        }

        func verify(snapshot: BsvHomeTransferPasswordSnapshot, password: String) async throws {
            switch self {
            case let .online(service): try await service.verify(snapshot: snapshot, password: password)
            case let .offline(service): try await offMain { try await service.verify(snapshot: snapshot, password: password) }
            }
        }

        func cancel() {
            switch self {
            case let .online(service): service.cancel()
            case let .offline(service): service.cancel()
            }
        }

        private func offMain<Value: Sendable>(_ action: @escaping @Sendable () async throws -> Value) async throws -> Value {
            try Task.checkCancellation()
            let worker = Task.detached(priority: .userInitiated) {
                try Task.checkCancellation()
                return try await action()
            }
            return try await withTaskCancellationHandler {
                try await worker.value
            } onCancel: {
                cancel()
                worker.cancel()
            }
        }
    }
}
