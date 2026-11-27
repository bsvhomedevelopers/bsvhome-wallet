// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct SecretDataTypeView: View {
    private let type: SecretPhraseDataType
    private let style: SecretPhraseGridView.Style

    public init(type: SecretPhraseDataType, style: SecretPhraseGridView.Style = .standard) {
        self.type = type
        self.style = style
    }

    public var body: some View {
        switch type {
        case let .words(rows):
            SecretPhraseGridView(rows: rows, style: style)
        case let .privateKey(key):
            Text(key)
                .font(style == .wowPay ? .wowPay.body : nil)
                .foregroundStyle(style == .wowPay ? AnyShapeStyle(Colors.WowPay.heading) : AnyShapeStyle(.foreground))
                .fixedSize(horizontal: false, vertical: style == .wowPay)
                .multilineTextAlignment(.center)
                .padding(.horizontal, .medium)
                .frame(maxWidth: .scene.content.maxWidth)
        }
    }
}
