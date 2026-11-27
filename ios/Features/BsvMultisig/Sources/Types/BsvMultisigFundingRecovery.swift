import struct BsvHomeCore.BsvHomeBsvMultisigFundingResult
import struct BsvHomeCore.BsvHomeBsvMultisigUnsubmittedFunding

struct BsvMultisigFundingRecovery: Sendable {
    let results: [BsvHomeBsvMultisigFundingResult]
    let unsubmitted: [BsvHomeBsvMultisigUnsubmittedFunding]
}
