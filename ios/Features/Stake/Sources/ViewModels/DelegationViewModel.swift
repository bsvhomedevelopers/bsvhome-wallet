// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import Foundation
import func BsvHomeCore.delegationStatus
import struct BsvHomeCore.BsvHomeDelegationStatus
import protocol BsvHomeCore.BsvHomeStakeServiceProtocol
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct DelegationViewModel: Sendable {
    public let delegation: Delegation
    public let currencyCode: String
    private let asset: Asset
    private let formatter: ValueFormatter
    private let service: any BsvHomeStakeServiceProtocol
    private let priceViewModel: PriceViewModel

    private static let dateFormatterDefault: DateComponentsFormatter = {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = [.day, .hour]
        formatter.zeroFormattingBehavior = .dropLeading
        formatter.unitsStyle = .full
        return formatter
    }()

    private static let dateFormatterDay: DateComponentsFormatter = {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = [.hour, .minute]
        formatter.zeroFormattingBehavior = .dropLeading
        formatter.unitsStyle = .full
        return formatter
    }()

    public init(
        service: any BsvHomeStakeServiceProtocol,
        delegation: Delegation,
        asset: Asset,
        formatter: ValueFormatter = .short,
        currencyCode: String,
    ) {
        self.delegation = delegation
        self.currencyCode = currencyCode
        self.asset = asset
        self.formatter = formatter
        self.service = service
        priceViewModel = PriceViewModel(price: delegation.price, currencyCode: currencyCode)
    }

    public var status: BsvHomeDelegationStatus {
        delegationStatus(delegation: delegation.map())
    }

    public var stateModel: DelegationStateViewModel {
        DelegationStateViewModel(status: status)
    }

    public var titleStyle: TextStyle {
        TextStyle(font: .body, color: .primary, fontWeight: .semibold)
    }

    public var subtitleStyle: TextStyle {
        TextStyle(font: .callout, color: Colors.black, fontWeight: .semibold)
    }

    public var subtitleExtraStyle: TextStyle {
        TextStyle(font: .footnote, color: Colors.gray)
    }

    public var balanceText: String {
        formatter.balanceString(delegation.base.balance, asset: asset, currency: asset.symbol)
    }

    public var fiatValueText: String? {
        priceViewModel.fiatAmountText(value: delegation.base.balance, decimals: asset.decimals.asInt)
    }

    private var showsRewards: Bool {
        service.showsRewards(delegation: delegation.base.map())
    }

    public var rewardsText: String? {
        guard showsRewards else { return nil }
        return formatter.string(delegation.base.rewards, decimals: asset.decimals.asInt, currency: asset.symbol)
    }

    public var rewardsFiatValueText: String? {
        guard showsRewards else { return nil }
        return priceViewModel.fiatAmountText(value: delegation.base.rewards, decimals: asset.decimals.asInt)
    }

    public var validatorModel: ValidatorViewModel {
        ValidatorViewModel(validator: delegation.validator)
    }

    public var validatorText: String {
        validatorModel.name
    }

    public var validatorImage: AssetImage {
        validatorModel.validatorImage
    }

    public var validatorUrl: URL? {
        service.validatorUrl(validator: delegation.validator.map()).map { $0.map() }?.url
    }

    public var completionDateText: String? {
        guard status.completion != nil else { return nil }
        let now = Date.now
        if let completionDate = delegation.base.completionDate, completionDate > now {
            if now.distance(to: completionDate) < 86400 {
                return Self.dateFormatterDay.string(from: .now, to: completionDate)
            }
            return Self.dateFormatterDefault.string(from: .now, to: completionDate)
        }
        return .none
    }
}

extension DelegationViewModel: Identifiable {
    public var id: String {
        delegation.id
    }
}

// MARK: - ValueHeaderViewModel

extension DelegationViewModel: ValueHeaderViewModel {
    public var isWatchWallet: Bool {
        false
    }

    public var buttons: [HeaderButton] {
        []
    }

    public var assetImage: AssetImage? {
        validatorImage
    }

    public var title: String {
        balanceText
    }

    public var subtitle: String? {
        fiatValueText
    }

    public var subtitleColor: Color {
        .secondary
    }
}
