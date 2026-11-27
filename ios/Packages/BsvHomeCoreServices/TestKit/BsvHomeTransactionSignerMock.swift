// Copyright (c). Gem Wallet. All rights reserved.

public import struct BsvHomeCore.BsvHomeSignedTransaction
public import struct BsvHomeCore.BsvHomeSignerInput
public import protocol BsvHomeCore.BsvHomeTransactionSigner
public import struct BsvHomeCore.Wallet
public import BsvHomeCorePrimitives
import Primitives

public final class BsvHomeTransactionSignerMock: BsvHomeTransactionSigner {
    public static let transferType = Primitives.TransactionType.transfer.map()

    public let signedTransactions: [BsvHomeSignedTransaction]

    public init(signedTransactions: [BsvHomeSignedTransaction] = [BsvHomeSignedTransaction(data: "signed_data", transactionType: BsvHomeTransactionSignerMock.transferType)]) {
        self.signedTransactions = signedTransactions
    }

    public func sign(wallet _: BsvHomeCore.Wallet, input _: BsvHomeSignerInput) throws -> [BsvHomeSignedTransaction] {
        signedTransactions
    }
}

public extension BsvHomeSignedTransaction {
    init(data: String, type: Primitives.TransactionType) {
        self.init(data: data, transactionType: type.map())
    }
}
