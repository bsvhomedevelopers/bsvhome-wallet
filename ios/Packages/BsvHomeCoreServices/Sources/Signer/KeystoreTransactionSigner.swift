// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.BsvHomeSignedTransaction
public import struct BsvHomeCore.BsvHomeSignerInput
public import protocol BsvHomeCore.BsvHomeTransactionSigner
public import struct BsvHomeCore.Wallet
import enum BsvHomeCore.BsvHomeCoreError
import BsvHomeCorePrimitives
import Primitives

public final class KeystoreTransactionSigner: BsvHomeTransactionSigner {
    private let keystore: any Keystore

    public init(keystore: any Keystore) {
        self.keystore = keystore
    }

    public func sign(wallet: BsvHomeCore.Wallet, input: BsvHomeSignerInput) async throws -> [BsvHomeSignedTransaction] {
        do {
            return try await keystore.sign(wallet: wallet.map(), input: input)
        } catch where error.isAuthenticationCancelled {
            throw BsvHomeCoreError.Cancelled
        }
    }
}
