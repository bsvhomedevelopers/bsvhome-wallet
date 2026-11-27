// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Style
import SwiftUI

public protocol AssetPreviewable {
    var assetImage: AssetImage { get }
    var name: String { get }
    var subtitleSymbol: String? { get }
}

public struct AssetPreviewView<Model: AssetPreviewable>: View {
    public enum SubtitleLayout {
        case horizontal
        case vertical
    }

    public enum Style {
        case standard
        case wowPay
    }

    private let style: Style
    private let model: Model
    private let subtitleLayout: SubtitleLayout

    public init(model: Model, subtitleLayout: SubtitleLayout = .horizontal, style: Style = .standard) {
        self.style = style
        self.model = model
        self.subtitleLayout = subtitleLayout
    }

    public var body: some View {
        switch style {
        case .standard:
            standardContent
        case .wowPay:
            HStack(spacing: Spacing.space12) {
                AssetImageView(assetImage: model.assetImage, size: Sizing.wowPay.detailAssetIcon)
                VStack(alignment: .leading, spacing: .tiny) {
                    Text(model.name)
                        .font(.wowPay.assetHeading)
                        .foregroundStyle(Colors.WowPay.heading)
                    if let symbol = model.subtitleSymbol {
                        Text(symbol)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var standardContent: some View {
        VStack(spacing: .medium) {
            AssetImageView(assetImage: model.assetImage, size: .image.semiLarge)

            layout {
                Text(model.name)
                    .textStyle(.headline)
                    .lineLimit(1)
                if let symbol = model.subtitleSymbol {
                    Text(symbol)
                        .textStyle(TextStyle(font: .subheadline, color: Colors.secondaryText, fontWeight: .medium))
                        .lineLimit(1)
                }
            }
        }
    }

    private var layout: AnyLayout {
        switch subtitleLayout {
        case .horizontal: AnyLayout(HStackLayout(alignment: .lastTextBaseline, spacing: .tiny))
        case .vertical: AnyLayout(VStackLayout(spacing: .tiny))
        }
    }
}
