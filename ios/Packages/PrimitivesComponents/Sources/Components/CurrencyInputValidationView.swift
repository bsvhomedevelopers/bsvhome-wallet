// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Style
import SwiftUI
import Validators

public struct CurrencyInputValidationView: View {
    @Binding private var model: InputValidationViewModel

    private let style: CurrencyInputStyle
    private let config: CurrencyInputConfigurable
    private let infoAction: (any Error) -> (() -> Void)?

    public init(
        model: Binding<InputValidationViewModel>,
        config: CurrencyInputConfigurable,
        style: CurrencyInputStyle = .standard,
        infoAction: @escaping (any Error) -> (() -> Void)? = { _ in nil },
    ) {
        _model = model
        self.config = config
        self.style = style
        self.infoAction = infoAction
    }

    public var body: some View {
        VStack(spacing: .small) {
            CurrencyInputView(
                text: $model.text,
                config: config,
                style: style,
            )

            if let error = model.error, !(error is SilentValidationError) {
                HStack {
                    if let action = infoAction(error) {
                        InfoButton(action: action)
                    }
                    Text(.init(error.localizedDescription))
                        .multilineTextAlignment(.center)
                        .textStyle(TextStyle(font: .footnote, color: Colors.red))
                        .transition(.opacity)
                }
            }
        }
    }
}
