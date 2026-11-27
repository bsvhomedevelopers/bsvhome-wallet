import BsvHomeCore

enum BsvMultisigFixtures {
    static func paymentRequest(amount: String = "") throws -> BsvHomeBsvMultisigPaymentRequest {
        let first = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        let second = "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
        return try bsvMultisigPaymentRequestFromScript(name: "Team", outputScript: "5221" + first + "21" + second + "52ae", amount: amount)
    }

    static func paymail(walletId: String = "multisig", canManage: Bool = true, active: Bool = true, version: UInt64 = 3) -> BsvHomeBsvMultisigPaymailSettings {
        BsvHomeBsvMultisigPaymailSettings(walletId: walletId, handle: "team@paymail.bsvhome.com", alias: "team", domain: "paymail.bsvhome.com", active: active, canManage: canManage, version: version, configurationHash: "public-configuration")
    }

    static func wallet(id: String = "multisig") -> BsvHomeBsvMultisigWallet {
        BsvHomeBsvMultisigWallet(id: id, name: "Team", threshold: 2, memberCount: 2, version: 9, status: .active, configurationStatus: .confirmed,
                             members: [], configurationId: "public-configuration", lockingScript: "public-script", localSignerWalletId: "member", localSignerPublicKey: "member-public-key",
                             balanceSatoshis: "100000", balanceUpdatedAt: 1, balanceIsStale: false, pendingCount: 1, canInvite: false, canTransact: true, personalConfigurationPending: false)
    }

    static func proposal(version: UInt64 = 7, canSign: Bool = true, canBroadcast: Bool = false, confirmations: UInt64? = 0) -> BsvHomeBsvMultisigProposal {
        BsvHomeBsvMultisigProposal(id: "proposal", walletId: "multisig", version: version, destination: "recipient-address", valueSatoshis: "42000", feeSatoshis: "200", memo: "Purpose",
                               state: canBroadcast ? .readyToBroadcast : .collectingSignatures, remoteStatus: canBroadcast ? "READY" : "COLLECTING", inputCount: 2,
                               signedMembers: [], requiredSignatures: 2, transactionId: nil, createdAt: 100, expiresAt: 1_000_000, confirmations: confirmations, canSign: canSign, canBroadcast: canBroadcast, canWithdraw: false, personalPending: true)
    }

    static func review(proposal: BsvHomeBsvMultisigProposal = BsvMultisigFixtures.proposal()) -> BsvHomeBsvMultisigProposalReview {
        BsvHomeBsvMultisigProposalReview(reviewId: "review-fixture", proposal: proposal, fields: [])
    }

    static func preparation() -> BsvHomeBsvMultisigFundingPreparation {
        BsvHomeBsvMultisigFundingPreparation(id: "review", target: .member(walletId: "multisig", name: "Team", members: [], threshold: 2), payerWalletId: "payer", payerWalletName: "Payer", senderAddress: "payer-address",
                                         amountSatoshis: "42000", feeSatoshis: "200", totalSatoshis: "42200", changeSatoshis: "57800", inputCount: 2, configurationHash: "public-configuration", remainingSeconds: 300)
    }

    static func result(status: BsvHomeBsvMultisigFundingStatus = .broadcast) -> BsvHomeBsvMultisigFundingResult {
        BsvHomeBsvMultisigFundingResult(target: .member(walletId: "multisig"), operationId: "funding-operation", payerWalletId: "payer", transactionId: "frozen-transaction-id", payerWalletName: "Payer",
                                    amountSatoshis: "42000", feeSatoshis: "200", totalSatoshis: "42200", status: status, errorCode: nil)
    }
}
