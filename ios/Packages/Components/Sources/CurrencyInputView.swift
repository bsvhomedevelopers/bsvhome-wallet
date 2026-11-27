// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public protocol CurrencyInputConfigurable {
    var placeholder: String { get }
    var currencySymbol: String { get }
    var currencyPosition: CurrencyTextField.CurrencyPosition { get }
    var secondaryText: String { get }
    var keyboardType: UIKeyboardType { get }
    var sanitizer: ((String) -> String)? { get }
    var actionStyle: CurrencyInputActionStyle? { get }
    var onTapActionButton: (() -> Void)? { get }
}

public struct CurrencyInputView: View {
    @Binding var text: String

    private let placeholder: String
    private let currencySymbol: String
    private let currencyPosition: CurrencyTextField.CurrencyPosition
    private let secondaryText: String
    private let keyboardType: UIKeyboardType
    private let actionStyle: CurrencyInputActionStyle?
    private let onTapActionButton: (() -> Void)?
    private let sanitizer: ((String) -> String)?
    private let style: CurrencyInputStyle

    public init(
        placeholder: String = "0",
        text: Binding<String>,
        currencySymbol: String,
        currencyPosition: CurrencyTextField.CurrencyPosition,
        secondaryText: String,
        keyboardType: UIKeyboardType,
        actionStyle: CurrencyInputActionStyle? = nil,
        onTapActionButton: (() -> Void)? = nil,
        sanitizer: ((String) -> String)? = nil,
        style: CurrencyInputStyle = .standard,
    ) {
        _text = text
        self.secondaryText = secondaryText
        self.placeholder = placeholder
        self.keyboardType = keyboardType
        self.currencySymbol = currencySymbol
        self.currencyPosition = currencyPosition
        self.actionStyle = actionStyle
        self.onTapActionButton = onTapActionButton
        self.sanitizer = sanitizer
        self.style = style
    }

    public init(text: Binding<String>, config: CurrencyInputConfigurable, style: CurrencyInputStyle = .standard) {
        self.init(
            placeholder: config.placeholder,
            text: text,
            currencySymbol: config.currencySymbol,
            currencyPosition: config.currencyPosition,
            secondaryText: config.secondaryText,
            keyboardType: config.keyboardType,
            actionStyle: config.actionStyle,
            onTapActionButton: config.onTapActionButton,
            sanitizer: config.sanitizer,
            style: style,
        )
    }

    public var body: some View {
        VStack(alignment: style == .wowPayTransfer ? .leading : .center, spacing: .small) {
            HStack(spacing: .small) {
                if let actionStyle, actionStyle.position == .amount {
                    actionButton(for: actionStyle.position)
                }

                CurrencyTextField(
                    placeholder,
                    text: $text,
                    currencySymbol: currencySymbol,
                    currencyPosition: currencyPosition,
                    keyboardType: keyboardType,
                    style: style,
                )
                .ifLet(sanitizer) { view, sanitize in
                    view.onChange(of: text) { _, newValue in
                        text = sanitize(newValue)
                    }
                }
            }

            if let actionStyle, actionStyle.position == .secondary {
                actionButton(for: actionStyle.position)
            } else if style == .standard || !secondaryText.isEmpty {
                secondaryTextView
            }
        }
        .frame(maxWidth: style == .wowPaySwap ? Sizing.wowPay.swapAmountMaxWidth : nil)
        .onAppear {
            UITextField.appearance().clearButtonMode = .never
        }
    }

    func actionButton(for position: CurrencyInputActionPosition) -> some View {
        Button {
            onTapActionButton?()
        } label: {
            switch position {
            case .amount:
                actionImage
            case .secondary:
                HStack {
                    secondaryTextView
                    actionImage
                }
            }
        }
        .buttonStyle(.plain)
        .disabled(actionStyle == nil)
    }

    @ViewBuilder
    var actionImage: some View {
        if let actionStyle {
            actionStyle.image
                .resizable()
                .frame(width: actionStyle.imageSize, height: actionStyle.imageSize)
                .foregroundStyle(Colors.gray)
        }
    }

    var secondaryTextView: some View {
        Text(secondaryText)
            .textStyle(style == .standard ? .calloutSecondary.weight(.medium) : .wowPay.value)
            .frame(minHeight: .list.image)
    }
}
