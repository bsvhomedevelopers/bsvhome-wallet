import struct BsvHomeCore.BsvHomeBsvMultisigFundingResult
import Localization

extension BsvHomeBsvMultisigFundingResult {
    var memberWalletId: String? {
        if case let .member(walletId) = target {
            walletId
        } else {
            nil
        }
    }

    func targetName(memberName: (String) -> String?) -> String {
        target.displayName(memberName: memberName)
    }
}
