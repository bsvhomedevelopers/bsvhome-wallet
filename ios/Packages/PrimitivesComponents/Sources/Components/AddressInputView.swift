// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Primitives
import Style
import SwiftUI

public struct AddressInputView: View {
    @Binding private var model: AddressInputViewModel

    private let style: FloatFieldStyle
    private let onSelectScan: (@MainActor () -> Void)?

    public init(
        model: Binding<AddressInputViewModel>,
        style: FloatFieldStyle = .standard,
        onSelectScan: (@MainActor () -> Void)? = nil,
    ) {
        _model = model
        self.style = style
        self.onSelectScan = onSelectScan
    }

    public var body: some View {
        InputValidationField(
            model: $model.inputModel,
            style: style,
            placeholder: model.placeholder,
            allowClean: true,
            trailingView: {
                HStack(spacing: Spacing.medium) {
                    NameRecordView(model: model.nameRecordViewModel)
                    if model.shouldShowInputActions {
                        ListButton(image: Images.System.paste, action: model.onSelectPaste)
                        if let onSelectScan {
                            ListButton(image: Images.System.qrCodeViewfinder, action: onSelectScan)
                        }
                    }
                }
            },
        )
        .keyboardType(.alphabet)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .onChange(of: model.text, model.onTextChange)
        .onChange(of: model.nameResolveState, model.onNameResolveStateChange)
    }
}
