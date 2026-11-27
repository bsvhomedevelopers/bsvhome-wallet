// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import enum BsvHomeCore.BsvHomeFiatAmountCheck
import struct BsvHomeCore.FiatQuote
import struct BsvHomeCore.FiatQuoteUrl
import protocol BsvHomeCore.BsvHomeFiatQuoteServiceProtocol
import struct BsvHomeCore.BsvHomeFiatSession
import Primitives

public extension BsvHomeFiatQuoteServiceProtocol {
    var currency: Primitives.Currency {
        Primitives.Currency(core: getCurrency())
    }

    func newSession(type: FiatQuoteType, amount: Int?) -> BsvHomeFiatSession {
        newSession(quoteType: type.map(), amount: amount.map { UInt32($0) })
    }

    func amountCheck(type: FiatQuoteType, amount: Double, quote: FiatQuote?, available: BigInt) -> BsvHomeFiatAmountCheck {
        amountCheck(quoteType: type.map(), amount: amount, quote: quote, available: BigUInt(available))
    }

    func quoteUrl(asset: Asset, quoteId: String) async throws -> FiatQuoteUrl {
        try await quoteUrl(assetId: asset.id.identifier, quoteId: quoteId)
    }
}

public extension BsvHomeFiatSession {
    var type: FiatQuoteType {
        quoteType.map()
    }
}
