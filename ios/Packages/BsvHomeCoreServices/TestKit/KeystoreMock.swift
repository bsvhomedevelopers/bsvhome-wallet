// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.BsvHomeSignedTransaction
public import struct BsvHomeCore.BsvHomeSignerInput
public import class BsvHomeCore.MessageSigner
public import BsvHomeCorePrimitives
import Foundation
import BsvHomeCoreServices
import Primitives

public struct KeystoreMock: Keystore {
    public init() {}

    public func keystorePassword(createIfMissing _: Bool) throws -> String {
        ""
    }

    public func migrateV3Keystores(for _: [Primitives.Wallet]) throws -> [KeystoreMigrationFailure] {
        []
    }

    public func sign(wallet _: Primitives.Wallet, input _: BsvHomeSignerInput) throws -> [BsvHomeSignedTransaction] {
        []
    }

    public func signMessage(signer _: MessageSigner, wallet _: Primitives.Wallet) throws -> String {
        .empty
    }

    public func getPasswordAuthentication() throws -> KeystoreAuthentication {
        .none
    }

    public func destroy() throws {}
}
