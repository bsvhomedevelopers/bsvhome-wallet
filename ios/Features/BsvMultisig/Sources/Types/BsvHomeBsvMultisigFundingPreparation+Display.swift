import struct BsvHomeCore.BsvHomeBsvMultisigFundingPreparation
import Localization

extension BsvHomeBsvMultisigFundingPreparation {
    var memberWalletId: String? {
        if case let .member(walletId, _, _, _) = target {
            walletId
        } else {
            nil
        }
    }

    var targetName: String {
        switch target {
        case let .member(_, name, _, _): name
        case let .public(request): request.name.isEmpty ? Localized.multisigFundingTarget : request.name
        }
    }
}
