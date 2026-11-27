import BsvHomeCore

enum BsvMultisigPublicFundingFixtures {
    static func preparation(id: String = "public-operation", request: BsvHomeBsvMultisigPaymentRequest, payer: Wallet) -> BsvHomeBsvMultisigFundingPreparation {
        BsvHomeBsvMultisigFundingPreparation(id: id, target: .public(request: request), payerWalletId: payer.id, payerWalletName: payer.name, senderAddress: payer.accounts[0].address,
                                         amountSatoshis: "42000", feeSatoshis: "200", totalSatoshis: "42200", changeSatoshis: "57800", inputCount: 2, configurationHash: "public-display-configuration", remainingSeconds: 300)
    }

    static func result(id: String = "public-operation", request: BsvHomeBsvMultisigPaymentRequest, payer: Wallet, status: BsvHomeBsvMultisigFundingStatus = .broadcast) -> BsvHomeBsvMultisigFundingResult {
        BsvHomeBsvMultisigFundingResult(target: .public(request: request), operationId: id, payerWalletId: payer.id, transactionId: String(repeating: "1", count: 64), payerWalletName: payer.name,
                                    amountSatoshis: "42000", feeSatoshis: "200", totalSatoshis: "42200", status: status, errorCode: nil)
    }

    static func unsubmitted(id: String = "unsigned-operation", request: BsvHomeBsvMultisigPaymentRequest, payer: Wallet) -> BsvHomeBsvMultisigUnsubmittedFunding {
        BsvHomeBsvMultisigUnsubmittedFunding(operationId: id, target: .public(request: request), payerWalletId: payer.id, payerWalletName: payer.name, senderAddress: payer.accounts[0].address,
                                         amountSatoshis: "42000", feeSatoshis: "200", totalSatoshis: "42200")
    }
}
