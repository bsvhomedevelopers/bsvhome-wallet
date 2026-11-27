import Foundation
import class BsvHomeCore.BsvHomeBsvColdWalletActionScope
import class BsvHomeCore.BsvHomeOfflineSigningPasswordService
import protocol BsvHomeCore.BsvHomeSecureStore
import enum BsvHomeCore.BsvHomeServiceError
import struct BsvHomeCore.Wallet
import Observation
import Transfer
import UIKit

@Observable
@MainActor
final class BsvColdWalletPasswordAuthorization {
    private struct Request {
        let id: UUID
        let scope: BsvHomeBsvColdWalletActionScope
    }

    private let passwordAuthorization = TransferPasswordAuthorization.offline()
    private let store: any BsvHomeSecureStore
    private let isForeground: @MainActor () -> Bool
    private var request: Request?

    init(store: any BsvHomeSecureStore, isForeground: @escaping @MainActor () -> Bool = { UIApplication.shared.applicationState == .active }) {
        self.store = store
        self.isForeground = isForeground
    }

    var presentation: TransferPasswordViewModel? { passwordAuthorization.presentation }
    var requestId: UUID? { request?.id }

    func authorize(wallet: Wallet, scope: BsvHomeBsvColdWalletActionScope) async throws {
        guard request == nil, isForeground(), !Task.isCancelled else {
            scope.cancel()
            throw BsvHomeServiceError.Cancelled
        }
        try scope.check()
        let id = UUID()
        request = Request(id: id, scope: scope)
        defer {
            if request?.id == id {
                request = nil
            }
        }
        do {
            try await passwordAuthorization.authorizeOffline(wallet: wallet, requestId: id, check: { [weak self] in
                guard let self, request?.id == id, isForeground() else { throw BsvHomeServiceError.Cancelled }
                try scope.check()
            }, makeService: { [store] in
                try BsvHomeOfflineSigningPasswordService.forScope(store: store, scope: scope)
            })
            guard request?.id == id, isForeground(), !Task.isCancelled else { throw BsvHomeServiceError.Cancelled }
            try scope.check()
        } catch {
            cancel(requestId: id)
            throw error
        }
    }

    func cancel(scopeId: String) {
        guard let request, request.scope.id() == scopeId else { return }
        cancel(requestId: request.id)
    }

    func cancel() {
        guard let request else { return }
        cancel(requestId: request.id)
    }

    func dismissPassword(requestId: UUID) {
        guard presentation != nil else { return }
        cancel(requestId: requestId)
    }

    private func cancel(requestId: UUID) {
        guard let request, request.id == requestId else { return }
        self.request = nil
        request.scope.cancel()
        passwordAuthorization.cancel(requestId: requestId)
    }
}
