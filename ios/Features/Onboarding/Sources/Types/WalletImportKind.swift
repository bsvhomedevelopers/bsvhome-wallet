// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeWalletImportKind
import Localization

extension BsvHomeWalletImportKind {
    var title: String {
        switch self {
        case .phrase: Localized.Common.phrase
        case .privateKey: Localized.Common.privateKey
        case .address: Localized.Common.address
        }
    }

    var description: String {
        switch self {
        case .phrase: Localized.Common.secretPhrase
        case .privateKey: Localized.Common.privateKey
        case .address: Localized.Common.address
        }
    }
}
