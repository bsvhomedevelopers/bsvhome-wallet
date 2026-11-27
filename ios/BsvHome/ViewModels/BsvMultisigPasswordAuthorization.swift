import Foundation
import class BsvHomeCore.BsvHomeBsvMultisigActionScope
import protocol BsvHomeCore.BsvHomeBsvMultisigAuthorization
import enum BsvHomeCore.BsvHomeBsvMultisigAuthorizationRequest
import enum BsvHomeCore.BsvHomeBsvMultisigError
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeServiceError
import enum BsvHomeCore.BsvHomeCoreError
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import Localization
import Observation
import Primitives
import Transfer
import UIKit

@Observable
@MainActor
final class BsvMultisigPasswordAuthorization: BsvHomeBsvMultisigAuthorization {
    private struct Request {
        let id: UUID
        let scope: BsvHomeBsvMultisigActionScope
    }

    private let passwordAuthorization: TransferPasswordAuthorization
    private let makeService: @MainActor (Primitives.Wallet) throws -> any BsvHomePortalTransferPasswordServiceProtocol
    private let isForeground: @MainActor () -> Bool
    private var request: Request?

    init(makeService: @escaping @MainActor (Primitives.Wallet) throws -> any BsvHomePortalTransferPasswordServiceProtocol, isForeground: @escaping @MainActor () -> Bool = { UIApplication.shared.applicationState != .background }) {
        self.makeService = makeService
        self.isForeground = isForeground
        passwordAuthorization = TransferPasswordAuthorization(makeService: { _ in throw BsvHomeServiceError.Cancelled })
    }

    var presentation: TransferPasswordViewModel? { passwordAuthorization.presentation }
    var requestId: UUID? { request?.id }

    func authorize(wallet: BsvHomeCore.Wallet, request _: BsvHomeBsvMultisigAuthorizationRequest, scope: BsvHomeBsvMultisigActionScope) async throws {
        guard request == nil, isForeground(), !Task.isCancelled else {
            scope.cancel()
            throw BsvHomeServiceError.Cancelled
        }
        try checkScope(scope)
        let id = UUID()
        request = Request(id: id, scope: scope)
        defer {
            if request?.id == id {
                request = nil
            }
        }
        do {
            try await passwordAuthorization.authorize(wallet: wallet, requestId: id, check: { [weak self] in
                guard let self, request?.id == id, isForeground() else { throw BsvHomeServiceError.Cancelled }
                try checkScope(scope)
            }, makeService: { [makeService] in try makeService(wallet.map()) })
            guard request?.id == id, isForeground(), !Task.isCancelled else { throw BsvHomeServiceError.Cancelled }
            try checkScope(scope)
        } catch {
            cancel(requestId: id)
            if let error = error as? BsvHomeServiceError {
                throw error
            }
            if case .Cancelled? = error as? BsvHomeCoreError {
                throw BsvHomeServiceError.Cancelled
            }
            throw BsvHomeServiceError.Core(msg: Localized.multisigErrorAuthentication)
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
        guard passwordAuthorization.presentation != nil else { return }
        cancel(requestId: requestId)
    }

    private func cancel(requestId: UUID) {
        guard let request, request.id == requestId else { return }
        self.request = nil
        request.scope.cancel()
        passwordAuthorization.cancel(requestId: requestId)
    }

    private func checkScope(_ scope: BsvHomeBsvMultisigActionScope) throws {
        do {
            try scope.check()
        } catch BsvHomeBsvMultisigError.Cancelled {
            throw BsvHomeServiceError.Cancelled
        } catch {
            throw BsvHomeServiceError.Store(msg: Localized.multisigErrorStorage)
        }
    }
}
