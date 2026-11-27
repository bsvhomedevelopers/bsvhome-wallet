// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct SetupWalletScene: View {
    enum Field: Int, Hashable {
        case name
    }

    @FocusState private var focusedField: Field?
    @State private var model: SetupWalletViewModel

    public init(model: SetupWalletViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.section) {
                AvatarView(
                    avatarImage: model.avatarAssetImage,
                    size: .image.extraLarge,
                    action: onSelectImage,
                )
                .padding(.vertical, Spacing.wowPay.page)

                FloatTextField(
                    Localized.Wallet.name,
                    text: $model.nameInput,
                    style: .wowPay,
                    allowClean: focusedField == .name,
                )
                .focused($focusedField, equals: .name)
                .wowPayInput()
            }
            .padding(Spacing.wowPay.page)
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("", systemImage: SystemImage.checkmark, action: model.onComplete)
            }
        }
        .wowPayBackground()
        .safeAreaButton {
            StateButton(
                text: Localized.Common.done,
                type: .wowPayPrimary(.normal),
                action: model.onComplete,
            )
            .accessibilityIdentifier("finishWalletSetup")
        }
        .bindQuery(model.query)
        .onChange(of: model.nameInput) { Task { await model.onChangeWalletName() } }
        .alertSheet($model.isPresentingAlertMessage)
        .navigationTitle(model.title)
        .toolbarTitleDisplayMode(.inline)
    }
}

// MARK: - Actions

extension SetupWalletScene {
    private func onSelectImage() {
        focusedField = nil
        model.onSelectImage()
    }
}
