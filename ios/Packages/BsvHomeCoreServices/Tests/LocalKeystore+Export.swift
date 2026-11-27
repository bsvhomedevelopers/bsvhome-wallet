// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
@testable import BsvHomeCoreServices
import Primitives

extension LocalKeystore {
    func exportedPrivateKey(wallet: Primitives.Wallet, chain: Primitives.Chain) async throws -> String {
        let password = try await getPassword()
        return try withV4Password(keystore: bsvHomeKeystore, password) { password in
            try bsvHomeKeystore.exportPrivateKey(keystoreId: bsvHomeKeystore.keystoreId(walletId: wallet.id.id), chain: chain.rawValue, password: password)
        }
    }

    func exportedWords(wallet: Primitives.Wallet) async throws -> [String] {
        let password = try await getPassword()
        return try withV4Password(keystore: bsvHomeKeystore, password) { password in
            try bsvHomeKeystore.exportRecoveryPhrase(keystoreId: bsvHomeKeystore.keystoreId(walletId: wallet.id.id), password: password)
        }
    }
}
