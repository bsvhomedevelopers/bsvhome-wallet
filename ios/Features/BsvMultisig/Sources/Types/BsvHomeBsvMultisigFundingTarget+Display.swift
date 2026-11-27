import enum BsvHomeCore.BsvHomeBsvMultisigFundingTarget
import Localization

extension BsvHomeBsvMultisigFundingTarget {
    func displayName(memberName: (String) -> String?) -> String {
        switch self {
        case let .member(walletId): memberName(walletId) ?? Localized.Common.notAvailable
        case let .public(request): request.name.isEmpty ? Localized.multisigFundingTarget : request.name
        }
    }
}
