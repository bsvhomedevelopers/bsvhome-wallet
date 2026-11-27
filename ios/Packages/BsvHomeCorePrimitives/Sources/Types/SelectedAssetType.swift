// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRecipientType
import Primitives

public enum SelectedAssetType: Sendable, Hashable, Identifiable {
    case send(BsvHomeRecipientType)
    case receive(ReceiveAssetType)
    case stake(Asset)
    case earn(Asset)
    case buy(Asset, amount: Int?)
    case sell(Asset, amount: Int?)
    case swap(Asset, Asset?)

    public var isAvailableForPresentation: Bool {
        switch self {
        case .stake: StakingConfiguration.isEnabled
        case .send, .receive, .earn, .buy, .sell, .swap: true
        }
    }

    public var id: String {
        switch self {
        case let .send(type): "send_\(type.identifier())"
        case let .receive(type): "receive_\(type.id)"
        case let .stake(asset): "stake_\(asset.id)"
        case let .earn(asset): "earn_\(asset.id)"
        case let .buy(asset, _): "buy_\(asset.id)"
        case let .sell(asset, _): "sell_\(asset.id)"
        case let .swap(fromAsset, toAsset): "swap_\(fromAsset.id)_\(toAsset?.id.identifier ?? "")"
        }
    }
}
