import Components
import Style
import SwiftUI

public struct WowPayAssetDetailHeader<Market: View>: View {
    private let image: AssetImage
    private let symbol: String
    private let name: String
    private let kind: String
    private let market: Market

    public init(image: AssetImage, symbol: String, name: String, kind: String, @ViewBuilder market: () -> Market) {
        self.image = image
        self.symbol = symbol
        self.name = name
        self.kind = kind
        self.market = market()
    }

    public var body: some View {
        HStack(spacing: Spacing.space12) {
            AssetImageView(assetImage: image, size: Sizing.wowPay.detailAssetIcon)
            VStack(alignment: .leading, spacing: Spacing.space2) {
                Text(symbol)
                    .font(.wowPay.assetHeading)
                    .foregroundStyle(Colors.WowPay.heading)
                Text(name)
                Text(kind)
            }
            .font(.wowPay.subtitle)
            .foregroundStyle(Colors.WowPay.secondaryText)
            .layoutPriority(1)
            Spacer(minLength: .zero)
            market
        }
        .padding(.top, Spacing.space8)
    }
}
