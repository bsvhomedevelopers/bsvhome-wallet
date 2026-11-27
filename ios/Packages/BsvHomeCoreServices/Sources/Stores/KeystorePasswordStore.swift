// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeKeystoreAuthentication
import protocol BsvHomeCore.BsvHomeKeystorePassword
import typealias BsvHomeCore.WalletId
import enum BsvHomeCore.BsvHomeServiceError
import Primitives

public final class BsvHomeCoreKeystorePassword: BsvHomeKeystorePassword, @unchecked Sendable {
    private let keystore: any Keystore

    public init(keystore: any Keystore) {
        self.keystore = keystore
    }

    public func getPassword(createIfMissing: Bool) throws -> String {
        do {
            return try keystore.keystorePassword(createIfMissing: createIfMissing)
        } catch where error.isAuthenticationCancelled {
            throw BsvHomeServiceError.Cancelled
        }
    }

    public func getWalletPassword(walletId _: BsvHomeCore.WalletId) throws -> String? {
        nil
    }

    public func deleteWalletPassword(walletId _: BsvHomeCore.WalletId) throws {}

    public func authentication() throws -> BsvHomeKeystoreAuthentication {
        switch try keystore.getPasswordAuthentication() {
        case .biometrics: .biometrics
        case .passcode: .passcode
        case .none: .none
        }
    }
}
