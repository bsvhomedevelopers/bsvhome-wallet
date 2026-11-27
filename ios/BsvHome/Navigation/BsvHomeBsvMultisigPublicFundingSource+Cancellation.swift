import enum BsvHomeCore.BsvHomeBsvMultisigPublicFundingSource

extension BsvHomeBsvMultisigPublicFundingSource {
    func cancelPaymailPreparation() {
        switch self {
        case .paymentRequest: break
        case let .paymail(_, preparation): preparation.cancel()
        }
    }
}
