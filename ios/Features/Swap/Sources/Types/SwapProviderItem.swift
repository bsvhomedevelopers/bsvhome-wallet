// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import struct BsvHomeCore.SwapperQuote
import enum BsvHomeCore.SwapProvider
import struct BsvHomeCore.SwapQuote
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct SwapProviderItem: Sendable {
    public let asset: Asset
    public let swapQuote: BsvHomeCore.SwapQuote
    public let selectedProvider: BsvHomeCore.SwapProvider?
    public let priceViewModel: PriceViewModel
    public let valueFormatter: ValueFormatter
    public let swapperQuote: SwapperQuote?

    public init(
        asset: Asset,
        swapQuote: BsvHomeCore.SwapQuote,
        selectedProvider: BsvHomeCore.SwapProvider?,
        priceViewModel: PriceViewModel,
        valueFormatter: ValueFormatter,
    ) {
        self.asset = asset
        self.swapQuote = swapQuote
        self.selectedProvider = selectedProvider
        self.priceViewModel = priceViewModel
        self.valueFormatter = valueFormatter
        swapperQuote = nil
    }

    public init?(
        asset: Asset,
        swapperQuote: BsvHomeCore.SwapperQuote?,
        selectedProvider: BsvHomeCore.SwapProvider?,
        priceViewModel: PriceViewModel,
        valueFormatter: ValueFormatter,
    ) {
        guard let swapQuote = swapperQuote?.swapQuote else { return nil }
        self.asset = asset
        self.swapQuote = swapQuote
        self.swapperQuote = swapperQuote
        self.selectedProvider = selectedProvider
        self.priceViewModel = priceViewModel
        self.valueFormatter = valueFormatter
    }

    private var amount: String {
        valueFormatter.string(BigInt(swapQuote.toValue), decimals: asset.decimals.asInt)
    }

    private var isSelected: Bool {
        selectedProvider == swapQuote.providerData.provider
    }

    private func fiatBalance() -> String {
        priceViewModel.fiatAmountText(value: BigInt(swapQuote.toValue), decimals: asset.decimals.asInt) ?? .empty
    }
}

// MARK: - SimpleListItemViewable

extension SwapProviderItem: SimpleListItemViewable {
    public var title: String {
        swapQuote.providerData.protocolName
    }

    public var titleStyle: TextStyle {
        TextStyle(font: .callout, color: Colors.black, fontWeight: .semibold)
    }

    public var subtitle: String? {
        [amount, asset.symbol].joined(separator: " ")
    }

    public var assetImage: AssetImage {
        AssetImage(
            placeholder: swapQuote.providerData.provider.map().image,
            chainPlaceholder: isSelected ? Images.Wallets.selected : nil,
        )
    }

    public var subtitleExtra: String? {
        fiatBalance()
    }

    public var subtitleStyle: TextStyle {
        TextStyle(font: .callout, color: Colors.black, fontWeight: .semibold)
    }

    public var subtitleStyleExtra: TextStyle {
        TextStyle(font: .footnote, color: Colors.gray)
    }
}

// MARK: - Identifiable

extension SwapProviderItem: Identifiable {
    public var id: String {
        [
            swapQuote.toValue.description,
            swapQuote.fromValue.description,
            swapQuote.providerData.provider.map().rawValue,
        ].joined(separator: "_")
    }
}

// MARK: - Hashable

extension SwapProviderItem: Hashable {
    public static func == (lhs: SwapProviderItem, rhs: SwapProviderItem) -> Bool {
        lhs.id == rhs.id
    }

    public func hash(into hasher: inout Hasher) {
        id.hash(into: &hasher)
    }
}
