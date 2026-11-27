import struct BsvHomeCore.BsvHomeBsvMultisigPaymentRequest
import enum BsvHomeCore.BsvHomeBsvMultisigPublicFundingSource

public enum BsvMultisigPublicFundingEntry: Sendable {
    case payment(BsvHomeBsvMultisigPublicFundingSource)
    case recovery

    var request: BsvHomeBsvMultisigPaymentRequest? {
        switch self {
        case let .payment(.paymentRequest(request)): request
        case let .payment(.paymail(_, preparation)): preparation.paymentRequest()
        case .recovery: nil
        }
    }
}
