// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import Style
import SwiftUI

public struct TransactionView: View {
    public enum Appearance {
        case standard
        case assetDetail
    }

    private let model: TransactionViewModel
    private let currency: String
    private let style: ListItemView.Style
    private let appearance: Appearance

    public init(model: TransactionViewModel, currency: String, style: ListItemView.Style = .standard, appearance: Appearance = .standard) {
        self.model = model
        self.currency = currency
        self.style = style
        self.appearance = appearance
    }

    public var body: some View {
        switch appearance {
        case .assetDetail:
            assetDetailContent
        case .standard:
            standardContent
        }
    }

    private var assetDetailContent: some View {
        HStack(spacing: Spacing.wowPay.grid) {
            detailIcon
            ListItemView(
                title: textValue(model.titleTextValue, font: .wowPay.label, color: Colors.WowPay.heading),
                titleExtra: TextValue(text: TransactionDateFormatter(date: model.transaction.transaction.createdAt).numericRow, style: .wowPay.caption),
                titleTag: textValue(model.titleTagTextValue, font: .wowPay.serviceCaption, color: Colors.WowPay.secondaryText),
                titleTagType: model.titleTagType,
                subtitle: textValue(model.detailAmountTextValue(currency: currency), font: .wowPay.transactionValue, color: Colors.WowPay.heading),
            )
        }
    }

    @ViewBuilder
    private var detailIcon: some View {
        switch model.transaction.transaction.type {
        case .transfer:
            Image(model.transaction.transaction.direction == .incoming ? "wowpay-receive" : "wowpay-send")
                .resizable().scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                .frame(width: Sizing.wowPay.transactionIcon, height: Sizing.wowPay.transactionIcon)
                .background(model.transaction.transaction.direction == .incoming ? Colors.WowPay.receiveBadge : Colors.WowPay.pale, in: Circle())
                .accessibilityHidden(true)
        case .swap:
            Image("wowpay-swap").resizable().scaledToFit()
                .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                .frame(width: Sizing.wowPay.transactionIcon, height: Sizing.wowPay.transactionIcon)
                .background(Colors.WowPay.pale, in: Circle())
                .accessibilityHidden(true)
        default:
            AssetImageView(assetImage: model.assetImage, size: Sizing.wowPay.transactionIcon)
        }
    }

    private var standardContent: some View {
        ListItemView(
            title: textValue(model.titleTextValue, font: .wowPay.label, color: Colors.WowPay.heading),
            titleExtra: textValue(model.titleExtraTextValue, font: .wowPay.serviceCaption, color: Colors.WowPay.secondaryText),
            titleTag: textValue(model.titleTagTextValue, font: .wowPay.serviceCaption, color: Colors.WowPay.secondaryText),
            titleTagType: model.titleTagType,
            subtitle: textValue(model.subtitleTextValue(currency: currency), font: .wowPay.transactionValue, color: Colors.WowPay.heading),
            subtitleExtra: textValue(model.subtitleExtraTextValue(currency: currency), font: .wowPay.serviceCaption, color: Colors.WowPay.secondaryText),
            imageStyle: ListItemImageStyle(
                assetImage: model.assetImage,
                imageSize: style == .wowPay ? Sizing.wowPay.transactionIcon : .image.asset,
                cornerRadiusType: .rounded,
            ),
        )
    }

    private func textValue(_ value: TextValue?, font: WowPayFont, color: Color) -> TextValue? {
        guard let value, style == .wowPay else { return value }
        let mappedStyle = value.style.wowPay(font: font, color: color)
        return TextValue(
            text: value.text,
            style: TextStyle(
                font: mappedStyle.font,
                color: appearance == .assetDetail && mappedStyle.color == Colors.green ? Colors.WowPay.positive : mappedStyle.color,
                fontWeight: mappedStyle.fontWeight,
                background: mappedStyle.background,
            ),
            lineLimit: value.lineLimit,
            truncationMode: value.truncationMode,
        )
    }
}

// MARK: - Previews

#Preview {
    let pendingTransactionMock = Transaction(
        id: TransactionId(chain: .smartChain, hash: "0xe5fb66cef0fb71fa75e0245484a40d17952cf46053724c6ac61209bf307d6e56"),
        assetId: .init(chain: .smartChain, tokenId: ""),
        from: "0x92abCE21234D71EC443E679f3a1feAFD3Fc830fB",
        to: "0x8d7460E51bCf4eD26877cb77E56f3ce7E9f5EB8F",
        contract: nil,
        type: .transfer,
        state: .pending,
        blockNumber: "39348339",
        sequence: "1",
        fee: "21000000000000",
        feeAssetId: .init(chain: .smartChain, tokenId: ""),
        value: "100000000000000",
        memo: nil,
        direction: .outgoing,
        utxoInputs: [],
        utxoOutputs: [],
        metadata: nil,
        createdAt: Date(),
    )
    let pendingTransactionExtendedMock = TransactionExtended(
        recordId: 1,
        transaction: pendingTransactionMock,
        asset: Primitives.Chain.smartChain.asset,
        feeAsset: Primitives.Chain.smartChain.asset,
        price: nil,
        feePrice: nil,
        assets: [],
        prices: [],
        fromAddress: AddressName(chain: .smartChain, address: "0x92abCE21234D71EC443E679f3a1feAFD3Fc830fB", name: "test1", type: .address, status: .verified, imageUrl: nil),
        toAddress: AddressName(chain: .smartChain, address: "0x8d7460E51bCf4eD26877cb77E56f3ce7E9f5EB8F", name: "test2", type: .address, status: .verified, imageUrl: nil),
        confirmationEtaSeconds: nil,
    )

    TransactionView(model: TransactionViewModel(transaction: pendingTransactionExtendedMock), currency: Currency.usd.rawValue, style: .wowPay)
}
