// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct BsvHomeCore.BsvHomeRecipient
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct RecipientScene: View {
    public enum Field: Int, Hashable, Identifiable {
        case address
        case memo
        public var id: String {
            String(rawValue)
        }
    }

    @FocusState private var focusedField: Field?

    private var model: RecipientSceneViewModel

    public init(model: RecipientSceneViewModel) {
        self.model = model
    }

    public var body: some View {
        @Bindable var model = model
        List {
            Group {
                switch model.type {
                case let .asset(asset):
                    AssetPreviewView(model: AssetViewModel(asset: asset.map()), style: .wowPay)
                case let .nft(nftAsset):
                    NftPreviewView(
                        assetImage: model.nftAssetImage(for: nftAsset.map()),
                        name: nftAsset.name,
                        size: .image.large,
                    )
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.bottom, Spacing.wowPay.card)
            .cleanListRow()

            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                Text(model.tittle)
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)

                AddressInputView(
                    model: $model.addressInputModel,
                    style: .wowPay,
                    onSelectScan: { model.onSelectScan(field: .address) },
                )
                .textStyle(.wowPay.value)
                .wowPayInput()
                .focused($focusedField, equals: .address)

                if model.showMemo {
                    VStack(alignment: .leading, spacing: .small) {
                        Text(model.memoField)
                            .font(.wowPay.label)
                            .foregroundStyle(Colors.WowPay.heading)

                        FloatTextField(
                            model.memoField,
                            text: $model.memo,
                            style: .wowPay,
                            allowClean: focusedField == .memo,
                            trailingView: {
                                ListButton(
                                    image: Images.System.qrCodeViewfinder,
                                    action: { model.onSelectScan(field: .memo) },
                                )
                            },
                        )
                        .textStyle(.wowPay.value)
                        .wowPayInput()
                        .focused($focusedField, equals: .memo)
                        .keyboardType(.alphabet)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    }
                }
            }
            .buttonStyle(.plain)
            .wowPayCard()
            .cleanListRow()

            ForEach(model.recipientSections) { section in
                Section {
                    ForEach(section.values) {
                        let recipient = $0.value
                        NavigationCustomLink(
                            with: ListItemView(
                                title: $0.title ?? $0.value.name,
                                titleStyle: .wowPay.rowTitle,
                                subtitle: $0.subtitle,
                                subtitleStyle: .wowPay.caption,
                            ),
                            action: { onSelectRecipient(recipient) },
                        )
                        .padding(.vertical, .small)
                        .listRowBackground(Colors.WowPay.surface)
                    }
                } header: {
                    HStack {
                        section.image
                            .frame(size: .image.small)
                        Text(section.section)
                    }
                    .textStyle(.wowPay.rowTitle)
                    .textCase(nil)
                }
            }
        }
        .safeAreaButton {
            StateButton(
                text: model.actionButtonTitle,
                type: .wowPayPrimary(model.actionButtonState),
                action: onSelectContinue,
            )
        }
        .listStyle(.insetGrouped)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.card, for: .scrollContent)
        .listSectionSpacing(.custom(Spacing.wowPay.card))
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.tittle)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(model.actionButtonTitle, action: onSelectContinue)
                    .bold()
                    .disabled(!model.isNextEnabled)
            }
        }
        .bindQuery(model.contactsQuery)
        .bindQuery(model.walletsQuery)
        .onChange(of: model.addressInputModel.text, model.onChangeAddressText)
    }
}

// MARK: - Actions

extension RecipientScene {
    private func onSelectRecipient(_ recipient: BsvHomeRecipient) {
        focusedField = nil
        model.onSelectRecipient(recipient)
    }

    private func onSelectContinue() {
        focusedField = nil
        model.onContinue()
    }
}
