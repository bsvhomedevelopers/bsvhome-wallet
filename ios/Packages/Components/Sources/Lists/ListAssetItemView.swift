import Style
import SwiftUI

public struct ListAssetItemView: View {
    public enum Style {
        case standard
        case wowPay
    }

    private let style: Style
    let model: ListAssetItemViewable
    @State private var toggleValue: Bool

    public init(model: ListAssetItemViewable, style: Style = .standard) {
        self.model = model
        self.style = style
        _toggleValue = State(wrappedValue: model.isAssetEnabled)
    }

    public var body: some View {
        rowContent
            .onChange(of: toggleValue) { _, newValue in
                model.action?(.switcher(enabled: newValue))
            }
    }
}

// MARK: - Components

extension ListAssetItemView {
    @ViewBuilder
    private var rowContent: some View {
        if style == .wowPay, case .balance = model.rightView {
            HStack(spacing: Spacing.wowPay.assetRowGap) {
                AssetImageView(assetImage: model.assetImage, size: Sizing.wowPay.assetIcon)
                ViewThatFits(in: .horizontal) {
                    HStack {
                        primaryContent.fixedSize(horizontal: true, vertical: true)
                        Spacer(minLength: .extraSmall)
                        secondaryContent.fixedSize(horizontal: true, vertical: true)
                    }
                    VStack(alignment: .leading, spacing: .small) {
                        primaryContent
                        secondaryContent.frame(maxWidth: .infinity, alignment: .trailing)
                    }
                    .fixedSize(horizontal: false, vertical: true)
                }
            }
        } else {
            ListItemFlexibleView(
                left: { AssetImageView(assetImage: model.assetImage, size: style == .wowPay ? Sizing.wowPay.assetIcon : .image.asset) },
                primary: { primaryContent },
                secondary: { secondaryContent },
            )
        }
    }

    private var wowPayCaptionStyle: TextStyle {
        TextStyle(font: .wowPay.caption, color: Colors.WowPay.secondaryText)
    }

    private var primaryContent: some View {
        VStack(alignment: .leading, spacing: style == .wowPay ? Spacing.wowPay.assetTextGap : .tiny) {
            headerView
            subtitleView
        }
    }

    private var headerView: some View {
        HStack(spacing: .tiny) {
            Text(model.name)
                .textStyle(
                    style == .wowPay
                        ? TextStyle(font: .wowPay.assetTitle, color: Colors.WowPay.heading)
                        : TextStyle(font: .body, color: .primary, fontWeight: .semibold),
                )
                .lineLimit(style == .wowPay ? nil : 1)
            if let symbol = model.symbol {
                Text(symbol)
                    .textStyle(style == .wowPay ? wowPayCaptionStyle : .calloutSecondary)
            }
        }
    }

    @ViewBuilder
    private var subtitleView: some View {
        switch model.subtitleView {
        case let .price(price, priceChangePercentage24h):
            if !price.text.isEmpty {
                HStack(spacing: .extraSmall) {
                    Text(price.text)
                        .textStyle(style == .wowPay ? wowPayCaptionStyle : price.style)
                    Text(priceChangePercentage24h.text)
                        .textStyle(style == .wowPay ? TextStyle(font: .wowPay.caption, color: priceChangePercentage24h.style.color) : priceChangePercentage24h.style)
                }
                .numericTransition(for: [price.text, priceChangePercentage24h.text])
            }
        case let .type(textValue):
            Text(textValue.text)
                .textStyle(style == .wowPay ? wowPayCaptionStyle : textValue.style)
                .numericTransition(for: textValue.text)
        case .none:
            EmptyView()
        }
    }

    @ViewBuilder
    private var secondaryContent: some View {
        switch model.rightView {
        case let .balance(balance, totalFiat):
            balanceView(balance: balance, totalFiat: totalFiat)
        case .toggle:
            Toggle("", isOn: $toggleValue)
                .labelsHidden()
                .toggleStyle(AppToggleStyle())
        case .copy:
            ListButton(
                image: Images.System.copy,
                padding: .small,
                action: {
                    model.action?(.copy)
                },
            )
            .background(Colors.grayVeryLight)
            .foregroundStyle(Colors.gray)
            .clipShape(RoundedRectangle(cornerRadius: 24))
        case .none:
            EmptyView()
        }
    }

    private func balanceView(balance: TextValue, totalFiat: TextValue) -> some View {
        VStack(alignment: .trailing, spacing: style == .wowPay ? Spacing.wowPay.assetTextGap : .tiny) {
            PrivacyText(balance.text, isEnabled: model.showBalancePrivacy)
                .textStyle(style == .wowPay ? TextStyle(font: .wowPay.assetValue, color: Colors.WowPay.heading) : balance.style)
                .numericTransition(for: balance.text)
            if !totalFiat.text.isEmpty {
                PrivacyText(totalFiat.text, isEnabled: model.showBalancePrivacy)
                    .textStyle(style == .wowPay ? wowPayCaptionStyle : totalFiat.style)
                    .numericTransition(for: totalFiat.text)
            }
        }
        .lineLimit(style == .wowPay ? nil : 1)
    }
}

// MARK: - Previews

#Preview {
    struct AssetListViewPreviewable: ListAssetItemViewable {
        let showBalancePrivacy: Binding<Bool>
        let name: String
        let symbol: String?
        let assetImage: AssetImage
        let subtitleView: ListAssetItemSubtitleView
        let rightView: ListAssetItemRightView
        var action: ((ListAssetItemAction) -> Void)?
    }

    return List {
        ListAssetItemView(
            model: AssetListViewPreviewable(
                showBalancePrivacy: .constant(false),
                name: "Bitcoin",
                symbol: "BTC",
                assetImage: AssetImage(
                    type: .text("ERC20"),
                    imageURL: URL(string: "https://assets.gemwallet.com/blockchains/bitcoin/info/logo.png")!,
                    placeholder: .none,
                    chainPlaceholder: .none,
                ),
                subtitleView: .none,
                rightView: .balance(
                    balance: TextValue(text: "test", style: TextStyle(font: .title, color: .accentColor)),
                    totalFiat: TextValue(text: "test2", style: TextStyle(font: .title, color: .accentColor)),
                ),
            ),
        )
    }
}
