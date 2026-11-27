import Foundation
import BsvHomeCore
import LocalAuthentication
import Primitives
import UIKit

public final class BsvHomeCoreBsvColdWalletSigner: BsvHomeBsvColdWalletSigner, @unchecked Sendable {
    private let keystore: LocalKeystore
    private let authenticator: any BiometryAuthenticatable
    private let reason: String
    private let authorizeSigning: @Sendable (BsvHomeCore.Wallet, BsvHomeBsvColdWalletActionScope) async throws -> Void

    public init(keystore: LocalKeystore, authenticator: any BiometryAuthenticatable, reason: String, authorizeSigning: @escaping @Sendable (BsvHomeCore.Wallet, BsvHomeBsvColdWalletActionScope) async throws -> Void) {
        self.keystore = keystore
        self.authenticator = authenticator
        self.reason = reason
        self.authorizeSigning = authorizeSigning
    }

    public func authorize(wallet: BsvHomeCore.Wallet, scope: BsvHomeBsvColdWalletActionScope) async throws {
        do {
            try await scope.validate()
            try Task.checkCancellation()
            try await authorizeSigning(wallet, scope)
            try Task.checkCancellation()
            try await scope.validate()
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch BsvHomeTransferPasswordError.Cancelled { throw BsvHomeServiceError.Cancelled }
        catch let error as BsvHomeServiceError { throw error }
        catch { throw BsvHomeServiceError.Platform(msg: "Offline signing password authorization failed") }
    }

    public func authenticate(wallet _: BsvHomeCore.Wallet, scope: BsvHomeBsvColdWalletActionScope) async throws {
        try await scope.validate()
        try await authenticateImport()
        try await scope.validate()
    }

    public func authenticateImport() async throws {
        do {
            try Task.checkCancellation()
            try await authenticator.authenticate(context: LAContext(), reason: reason)
            try Task.checkCancellation()
            try await requireForeground()
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch where error.isAuthenticationCancelled { throw BsvHomeServiceError.Cancelled }
        catch let error as BsvHomeServiceError { throw error }
        catch { throw BsvHomeServiceError.Platform(msg: "Device authentication failed") }
    }

    public func signRequest(wallet: BsvHomeCore.Wallet, requestJson: String, reviewJson: String, scope: BsvHomeBsvColdWalletActionScope) async throws -> String {
        do {
            try Task.checkCancellation()
            try await requireForeground()
            try await scope.validate()
            return try await keystore.signBsvColdRequest(wallet: wallet, requestJSON: requestJson, reviewJSON: reviewJson, scope: scope)
        } catch is CancellationError { throw BsvHomeServiceError.Cancelled }
        catch let error as BsvHomeServiceError { throw error }
        catch { throw BsvHomeServiceError.Platform(msg: "Cold wallet signing failed") }
    }

    @MainActor
    private func requireForeground() throws {
        guard UIApplication.shared.applicationState == .active, UIApplication.shared.isProtectedDataAvailable, !UIScreen.main.isCaptured else {
            throw BsvHomeServiceError.Cancelled
        }
    }
}
