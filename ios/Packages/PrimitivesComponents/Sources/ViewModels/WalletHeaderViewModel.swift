// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import enum BsvHomeCore.BsvHomeHeaderActions
import Primitives
import Style
import SwiftUI

public struct WalletHeaderViewModel {
    private let totalValue: TotalFiatValue
    private let actions: BsvHomeHeaderActions
    private let walletType: WalletType?
    private let totalValueViewModel: TotalValueViewModel

    public init(
        totalValue: TotalFiatValue,
        currencyCode: String,
        showsPnl: Bool,
        actions: BsvHomeHeaderActions,
        walletType: WalletType? = nil,
    ) {
        self.totalValue = totalValue
        self.actions = actions
        self.walletType = walletType
        let formatter = CurrencyFormatter(type: .fiat, currencyCode: currencyCode)
        totalValueViewModel = TotalValueViewModel(totalValue: totalValue, currencyFormatter: formatter, showsPnl: showsPnl)
    }
}

// MARK: - ValueHeaderViewModel

extension WalletHeaderViewModel: ValueHeaderViewModel {
    public var isWatchWallet: Bool {
        if let walletType {
            return walletType == .view
        }
        return actions == .watchOnly
    }

    public var title: String {
        totalValueViewModel.title
    }

    public var assetImage: AssetImage? {
        .none
    }

    public var subtitle: String? {
        guard let amount = totalValueViewModel.pnlAmountText else { return nil }
        guard let percentage = totalValueViewModel.pnlPercentageText else { return amount }
        return "\(amount) (\(percentage))"
    }

    public var subtitleColor: Color {
        totalValueViewModel.pnlColor
    }

    public var subtitleImage: Image? {
        Image(systemName: SystemImage.chartLineUptrendXyaxis)
    }

    public func isEnabled(_ type: HeaderButtonType) -> Bool {
        buttons.contains { $0.type == type && $0.isEnabled }
    }

    public var buttons: [HeaderButton] {
        switch actions {
        case .watchOnly: []
        case let .buttons(buttons): buttons.map { HeaderButton(type: $0.kind.headerButtonType, isEnabled: $0.isEnabled) }
        }
    }
}
