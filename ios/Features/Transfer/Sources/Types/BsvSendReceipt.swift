// Copyright (c). Gem Wallet. All rights reserved.

public struct BsvSendReceipt: Sendable {
    public let recipient: String
    public let amountText: String
    public let transactionId: String
}
