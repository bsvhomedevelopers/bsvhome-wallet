// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

public struct PrivacyText: View {
    public enum Layout {
        case adaptive
        case preserved
    }

    @Binding private var isEnabled: Bool

    private let text: String
    private let placeholder: String
    private let layout: Layout

    public init(
        _ text: String,
        isEnabled: Binding<Bool>,
        placeholder: String = "∗∗∗∗∗",
        layout: Layout = .adaptive,
    ) {
        self.text = text
        self.placeholder = placeholder
        self.layout = layout
        _isEnabled = isEnabled
    }

    public var body: some View {
        switch layout {
        case .adaptive:
            Text(displayText)
        case .preserved:
            ZStack(alignment: .leading) {
                Text(text)
                    .opacity(isEnabled ? 0 : 1)
                    .accessibilityHidden(isEnabled)
                Text(placeholder)
                    .opacity(isEnabled ? 1 : 0)
                    .accessibilityHidden(!isEnabled)
            }
        }
    }

    private var displayText: String {
        isEnabled ? placeholder : text
    }
}
