// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Style
import SwiftUI

public struct ListItemImageView: View {
    private let style: ListItemView.Style
    public let title: String?
    public let subtitle: String?
    public let subtitleStyle: TextStyle
    public let assetImage: AssetImage?
    public let assetImageStyle: AssetImageView.Style?
    public let imageSize: CGFloat
    public let infoAction: (() -> Void)?

    public init(
        title: String?,
        subtitle: String?,
        subtitleStyle: TextStyle = .calloutSecondary,
        assetImage: AssetImage? = nil,
        assetImageStyle: AssetImageView.Style? = nil,
        imageSize: CGFloat = .list.image,
        infoAction: (() -> Void)? = nil,
        style: ListItemView.Style = .standard,
    ) {
        self.style = style
        self.title = title
        self.subtitle = subtitle
        self.subtitleStyle = subtitleStyle
        self.assetImage = assetImage
        self.assetImageStyle = assetImageStyle
        self.imageSize = imageSize
        self.infoAction = infoAction
    }

    public init(model: ListItemModel, style: ListItemView.Style = .standard) {
        self.init(
            title: model.title,
            subtitle: model.subtitle,
            subtitleStyle: model.subtitleStyle,
            assetImage: model.imageStyle?.assetImage,
            imageSize: model.imageStyle?.imageSize ?? .list.image,
            infoAction: model.infoAction,
            style: style,
        )
    }

    public var body: some View {
        HStack {
            ListItemView(
                title: title,
                titleStyle: style == .standard ? .body : .wowPay.rowTitle,
                subtitle: subtitle,
                subtitleStyle: style == .standard ? subtitleStyle : subtitleStyle.wowPay(font: .wowPay.subtitle, color: Colors.WowPay.secondaryText),
                infoAction: infoAction,
            )
            if let assetImage {
                AssetImageView(assetImage: assetImage, size: imageSize, style: assetImageStyle)
            }
        }
    }
}
