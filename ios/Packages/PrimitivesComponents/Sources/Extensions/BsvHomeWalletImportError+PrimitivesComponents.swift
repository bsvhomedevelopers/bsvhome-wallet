// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeWalletImportError
import Localization

extension BsvHomeWalletImportError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .InvalidSecretPhrase:
            Localized.Errors.Import.invalidSecretPhrase
        case let .InvalidSecretPhraseWords(words):
            Localized.Errors.Import.invalidSecretPhraseWord(words.joined(separator: ", "))
        case .InvalidPrivateKey:
            Localized.Errors.Import.invalidPrivateKey
        case .InvalidAddress:
            Localized.Errors.invalidAddressName
        case .MissingChain:
            Localized.Errors.unknown
        }
    }
}
