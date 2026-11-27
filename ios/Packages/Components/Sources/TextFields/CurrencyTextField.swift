// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct CurrencyTextField: View {
    @Environment(\.isEnabled) var isEnabled
    @ScaledMetric private var fontSize: CGFloat

    public enum CurrencyPosition {
        case leading
        case trailing
    }

    private enum Constants {
        static let minWidth: CGFloat = 40
    }

    @Binding var text: String

    private let currencySymbol: String
    private let placeholder: String
    private let keyboardType: UIKeyboardType
    private let currencyPosition: CurrencyPosition
    private let style: CurrencyInputStyle

    public init(
        _ placeholder: String,
        text: Binding<String>,
        currencySymbol: String,
        currencyPosition: CurrencyPosition,
        keyboardType: UIKeyboardType,
        style: CurrencyInputStyle = .standard,
    ) {
        _text = text
        self.placeholder = placeholder
        self.keyboardType = keyboardType
        self.currencySymbol = currencySymbol
        self.currencyPosition = currencyPosition
        self.style = style
        switch style.font {
        case let .scaled(size, _, relativeTo):
            _fontSize = ScaledMetric(wrappedValue: size, relativeTo: relativeTo)
        case .native:
            _fontSize = ScaledMetric(wrappedValue: 0)
        }
    }

    public var body: some View {
        HStack(alignment: .center, spacing: .zero) {
            if currencyPosition == .leading {
                currencySymbolView
            }
            TextField(placeholder, text: $text)
                .keyboardType(keyboardType)
                .foregroundStyle(style.foregroundColor)
                .font(style.font)
                .multilineTextAlignment(style.textAlignment)
                .textFieldStyle(.plain)
                .lineLimit(1)
                .padding(currencyPosition == .leading ? .leading : .trailing, style == .standard ? .tiny : .zero)
                .frame(minWidth: Constants.minWidth, maxWidth: style.maximumWidth)
                .fixedSize(horizontal: style == .standard, vertical: false)
                .disabled(!isEnabled)

            if currencyPosition == .trailing {
                currencySymbolView
            }
        }
        .frame(height: style.height(scaledFontSize: fontSize))
    }

    private var currencySymbolView: some View {
        Text(currencySymbol)
            .font(style.font)
            .foregroundStyle(style.foregroundColor)
            .lineLimit(1)
            .fixedSize()
    }
}

#Preview {
    @Previewable @State var textLeading = "10"
    @Previewable @State var textTrailing = "100"

    return VStack {
        CurrencyTextField("0", text: $textLeading, currencySymbol: "$", currencyPosition: .leading, keyboardType: .numberPad)
        CurrencyTextField("0", text: $textTrailing, currencySymbol: "$", currencyPosition: .trailing, keyboardType: .decimalPad)
    }
}
