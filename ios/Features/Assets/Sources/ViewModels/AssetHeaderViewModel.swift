// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import struct BsvHomeCore.BsvHomeAssetDetailsState
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct AssetHeaderViewModel {
    let assetDataModel: AssetDataViewModel
    let state: BsvHomeAssetDetailsState
}

extension AssetHeaderViewModel: ValueHeaderViewModel {
    var isWatchWallet: Bool {
        state.headerActions == .watchOnly
    }

    var assetImage: AssetImage? {
        assetDataModel.assetImage
    }

    var title: String {
        assetDataModel.totalBalanceTextWithSymbol
    }

    var amountTitle: String { assetDataModel.balanceText }

    var amountSymbol: String? { assetDataModel.symbol }

    var subtitle: String? {
        guard assetDataModel.isPriceAvailable else { return "—" }
        return assetDataModel.fiatBalanceText.isEmpty ? assetDataModel.priceViewModel.fiatAmountText(amount: .zero) : assetDataModel.fiatBalanceText
    }

    var subtitleColor: Color {
        Colors.gray
    }

    var buttons: [HeaderButton] {
        switch state.headerActions {
        case .watchOnly: []
        case let .buttons(buttons): buttons.map { HeaderButton(type: $0.kind.headerButtonType, isEnabled: $0.isEnabled) }
        }
    }
}
