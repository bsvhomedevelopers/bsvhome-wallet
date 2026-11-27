// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import struct Stake.ValidatorView
import struct Stake.ValidatorViewModel
import Style
import SwiftUI

public struct AmountScene: View {
    @FocusState private var focusedField: Bool

    private var model: AmountSceneViewModel

    public init(model: AmountSceneViewModel) {
        self.model = model
    }

    public var body: some View {
        @Bindable var model = model
        List {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(Localized.wowpayAmount)
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)

                CurrencyInputValidationView(
                    model: $model.amountInputModel,
                    config: model.inputConfig,
                    style: .wowPayTransfer,
                    infoAction: model.infoAction(for:),
                )
                .wowPayInput()
                .disabled(model.isInputDisabled)
                .focused($focusedField)

                if model.isBalanceViewEnabled {
                    AssetBalanceView(
                        image: model.assetImage,
                        title: model.assetName,
                        balance: model.balanceText,
                        style: .wowPay,
                        secondary: {
                            Button(model.maxTitle, action: onSelectMaxButton)
                                .buttonStyle(.wowPaySecondary())
                                .fixedSize()
                        },
                    )
                }
            }
            .wowPayCard()
            .cleanListRow()

            if let infoText = model.infoText {
                Section {
                    Button(action: model.onSelectReservedFeesInfo) {
                        HStack(spacing: .space12) {
                            Images.System.info
                                .foregroundStyle(Colors.WowPay.brand)
                                .frame(size: Sizing.wowPay.icon)
                            Text(infoText)
                                .textStyle(.wowPay.caption)
                                .multilineTextAlignment(.leading)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .wowPayCard(surface: Colors.WowPay.pale)
                    }
                    .buttonStyle(.plain)
                    .cleanListRow()
                }
            }

            switch model.provider {
            case let .stake(stake):
                switch stake.selection {
                case let .validator(validatorSelection):
                    Section(validatorSelection.title) {
                        if validatorSelection.isEnabled {
                            NavigationLink(value: validatorSelection.selected) {
                                ValidatorView(model: ValidatorViewModel(validator: validatorSelection.selected))
                            }
                        } else {
                            ValidatorView(model: ValidatorViewModel(validator: validatorSelection.selected))
                        }
                    }

                case let .resource(resourceSelection):
                    @Bindable var resourceSelection = resourceSelection
                    Section {
                        Picker("", selection: $resourceSelection.selected) {
                            ForEach(resourceSelection.options) { resource in
                                Text(ResourceViewModel(resource: resource).title)
                                    .tag(resource)
                            }
                        }
                        .pickerStyle(.segmented)
                        .frame(width: 200)
                        .onChange(of: resourceSelection.selected, model.onChangeResource)
                    }
                    .cleanListRow()
                }

            case let .perpetual(perpetual):
                if let leverageSelection = perpetual.leverageSelection {
                    Section {
                        NavigationCustomLink(
                            with: ListItemView(
                                title: leverageSelection.title,
                                subtitle: leverageSelection.selected.displayText,
                                subtitleStyle: perpetual.leverageTextStyle,
                            ),
                            action: model.onSelectLeverage,
                        )
                    }
                }

                if perpetual.isAutocloseEnabled {
                    Section {
                        NavigationCustomLink(
                            with: ListItemView(
                                title: perpetual.autocloseTitle,
                                subtitle: perpetual.autocloseText.subtitle,
                                subtitleExtra: perpetual.autocloseText.subtitleExtra,
                            ),
                            action: model.onSelectAutoclose,
                        )
                    }
                }

            case let .earn(earn):
                Section(earn.providerTitle) {
                    ValidatorView(model: ValidatorViewModel(validator: earn.provider))
                }

            case .transfer:
                EmptyView()
            }
        }
        .safeAreaButton {
            StateButton(
                text: model.continueTitle,
                type: .wowPayPrimary(model.actionButtonState),
                action: onSelectNextButton,
            )
        }
        .listStyle(.insetGrouped)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.card, for: .scrollContent)
        .listSectionSpacing(.custom(Spacing.wowPay.card))
        .frame(maxWidth: .infinity)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.title)
        .onChange(of: model.amountInputModel.text, model.onChangeAmountText)
        .onAppear {
            model.onAppear()
            if model.shouldFocusOnAppear {
                focusedField = true
            }
        }
    }

    private func onSelectMaxButton() {
        focusedField = false
        model.onSelectMaxButton()
    }

    private func onSelectNextButton() {
        focusedField = false
        model.onSelectNextButton()
    }
}
