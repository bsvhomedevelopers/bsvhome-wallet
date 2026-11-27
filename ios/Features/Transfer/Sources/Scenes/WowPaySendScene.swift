// Copyright (c). Gem Wallet. All rights reserved.

import Components
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

public struct WowPaySendScene: View {
    @FocusState private var focusedField: Field?
    @State private var model: WowPaySendViewModel
    @State private var isPresentingFeeInfo = false

    private enum Field: Hashable {
        case recipient
        case amount
    }

    public init(model: WowPaySendViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: Spacing.space12) {
                assetHeader
                    .padding(.bottom, Spacing.space4)
                formCard
                    .disabled(!model.isAddressPreparationComplete)
                feeCard
                if model.addressesState.isLoading {
                    LoadingView()
                } else if case let .error(error) = model.addressesState {
                    StateEmptyView(title: error.isCancelled ? Localized.Errors.cancelled : Localized.Errors.errorOccurred) {
                        Button(Localized.Common.tryAgain) {
                            Task { await model.prepareBsvAddresses() }
                        }
                        .buttonStyle(.wowPaySecondary())
                    }
                }
                StateButton(
                    text: Localized.wowpaySendConfirm,
                    type: .wowPayPrimary(model.isConfirmEnabled ? .normal : model.isEstimatingFee ? .loading() : .disabled),
                    action: onSelectConfirm,
                )
                .padding(.top, Spacing.space6)
                .accessibilityIdentifier("wowPaySendConfirm")
            }
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.top, Spacing.space8)
            .padding(.bottom, Spacing.wowPay.section)
        }
        .scrollDismissesKeyboard(.interactively)
        .wowPayBackground(.solid)
        .navigationTitle(Localized.Transfer.Send.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .bindQuery(model.assetQuery)
        .bindQuery(model.contactsQuery)
        .bindQuery(model.walletsQuery)
        .onChange(of: model.addressInputModel.text, model.onChangeAddressText)
        .onChange(of: model.amountInputModel.text, model.onChangeAmountText)
        .onChange(of: model.assetData, model.onChangeAssetData)
        .onAppear(perform: model.onSelectRetryFee)
        .onDisappear(perform: model.onDisappear)
        .task { await model.prepareBsvAddresses() }
        .task(id: model.contactsQuery.value) { await model.refreshContactRecipients() }
        .task(id: model.walletsQuery.value) { await model.refreshWalletRecipients(wallets: model.walletsQuery.value) }
        .sheet(isPresented: $model.isPresentingContacts) {
            contactPicker
        }
        .alert(Localized.Info.NetworkFee.title, isPresented: $isPresentingFeeInfo) {
            Button(Localized.Common.done, role: .cancel) {}
        } message: {
            Text(Localized.Info.NetworkFee.description(model.asset.chain.networkName, model.feeAssetSymbol))
        }
    }

    private var assetHeader: some View {
        HStack(spacing: Spacing.space12) {
            AssetImageView(
                assetImage: AssetIdViewModel(assetId: model.asset.id).assetImage,
                size: Sizing.wowPay.detailAssetIcon,
            )
            VStack(alignment: .leading, spacing: .tiny) {
                Text(model.asset.symbol)
                    .font(.wowPay.assetHeading)
                    .foregroundStyle(Colors.WowPay.heading)
                Text(model.asset.name)
                    .font(.wowPay.rate)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                Text(model.asset.type == .native
                    ? Localized.wowpayMainChainAsset
                    : AssetViewModel(asset: model.asset).networkFullName)
                    .font(.wowPay.rate)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            Spacer(minLength: .small)
            VStack(alignment: .trailing, spacing: .tiny) {
                Text(model.priceModel.isPriceAvailable ? model.priceModel.priceAmountText : Localized.Common.notAvailable)
                    .font(.wowPay.assetPrice)
                    .foregroundStyle(Colors.WowPay.heading)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                if model.priceModel.isPriceAvailable {
                    Text(model.priceModel.priceChangeText)
                        .font(.wowPay.rate.weight(.bold))
                        .foregroundStyle(model.priceModel.priceChangeTextColor)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var formCard: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            VStack(alignment: .leading, spacing: .small) {
                Text(model.asset.chain == .bitcoinSv
                    ? Localized.wowpaySendBsv
                    : Localized.wowpaySendAsset(model.asset.symbol))
                    .font(.wowPay.cardTitle)
                    .foregroundStyle(Colors.WowPay.heading)
                Text(model.asset.chain == .bitcoinSv
                    ? Localized.wowpaySendDescription
                    : Localized.wowpaySendAddressDescription(model.asset.chain.networkName))
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }

            VStack(alignment: .leading, spacing: .small) {
                Text(Localized.wowpaySendRecipientLabel)
                    .font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.heading)
                recipientInput
                recipientActions
                paymailResolution
                if let error = model.recipientError {
                    HStack(alignment: .top, spacing: .small) {
                        Text(error)
                            .foregroundStyle(Colors.red)
                        if model.canManagePaymailSender {
                            Button(Localized.wowpayPaymailManageSender, action: model.onSelectManagePaymail)
                                .foregroundStyle(Colors.WowPay.brand)
                                .accessibilityIdentifier("wowPaySendManagePaymail")
                        } else if model.isPaymailInput, model.supportsPaymail, !model.requiresPaymailSender {
                            Button(Localized.Common.tryAgain, action: model.onSelectRetryFee)
                                .foregroundStyle(Colors.WowPay.brand)
                        }
                    }
                    .font(.wowPay.caption)
                }
            }

            VStack(alignment: .leading, spacing: .small) {
                Text(Localized.wowpayAmount)
                    .font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.heading)
                amountInput
                if let error = model.amountError {
                    Text(error)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.red)
                }
                Text(model.amountEquivalent)
                    .font(.wowPay.rate)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                HStack(spacing: .tiny) {
                    Text(model.availableBalance)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                    Text("(\(model.availableBalanceEquivalent))")
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .font(.wowPay.rate)
                .lineLimit(1)
                .minimumScaleFactor(0.75)
                percentageButtons
            }
        }
        .wowPayCard()
    }

    private var recipientInput: some View {
        @Bindable var model = model
        return TextField(
            model.asset.chain == .bitcoinSv
                ? Localized.wowpaySendRecipientPlaceholder
                : Localized.wowpaySendAddressPlaceholder(model.asset.chain.networkName),
            text: $model.addressInputModel.text,
        )
        .font(.wowPay.sendRecipientInput)
        .foregroundStyle(Colors.WowPay.heading)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .keyboardType(.alphabet)
        .focused($focusedField, equals: .recipient)
        .accessibilityLabel(Localized.wowpaySendRecipientLabel)
        .accessibilityIdentifier("wowPaySendRecipient")
        .wowPayInput()
    }

    private var recipientActions: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: Spacing.small) { recipientActionButtons }
            VStack(alignment: .leading, spacing: .zero) { recipientActionButtons }
        }
        .buttonStyle(.plain)
        .font(.wowPay.caption)
        .foregroundStyle(Colors.WowPay.brand)
    }

    @ViewBuilder
    private var recipientActionButtons: some View {
        Button {
            focusedField = nil
            model.isPresentingScanner = true
        } label: {
            Text(Localized.wowpaySendScanQr)
                .padding(.horizontal, Spacing.space12)
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                .contentShape(Rectangle())
        }
        .accessibilityLabel(Localized.wowpaySendScanQr)
        .accessibilityIdentifier("wowPaySendScan")
        Button {
            focusedField = nil
            model.isPresentingContacts = true
        } label: {
            Text(Localized.Contacts.title)
                .padding(.horizontal, Spacing.space12)
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
                .contentShape(Rectangle())
        }
        .accessibilityLabel(Localized.Contacts.title)
        .accessibilityIdentifier("wowPaySendRecipients")
    }

    @ViewBuilder
    private var paymailResolution: some View {
        switch model.paymailResolutionState {
        case .none:
            EmptyView()
        case .resolving:
            Text(Localized.wowpayPaymailResolving)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.secondaryText)
                .accessibilityIdentifier("wowPaySendPaymailResolving")
        case .resolved:
            Text(Localized.wowpayPaymailResolved)
                .font(.wowPay.caption)
                .foregroundStyle(Colors.WowPay.brand)
                .accessibilityIdentifier("wowPaySendPaymailResolved")
        }
    }

    private var amountInput: some View {
        @Bindable var model = model
        return HStack(spacing: .small) {
            TextField(0.formatted(.number.precision(.fractionLength(2))), text: $model.amountInputModel.text)
                .font(.wowPay.sendAmountInput)
                .foregroundStyle(Colors.WowPay.heading)
                .keyboardType(.decimalPad)
                .focused($focusedField, equals: .amount)
                .accessibilityLabel(Localized.wowpayAmount)
                .accessibilityIdentifier("wowPaySendAmount")
            Text(model.asset.symbol)
                .foregroundStyle(Colors.WowPay.heading)
            Button(Localized.Common.all, action: model.onSelectMax)
                .foregroundStyle(Colors.WowPay.brand)
                .fixedSize()
        }
        .font(.wowPay.label)
        .wowPayInput()
    }

    private var percentageButtons: some View {
        HStack(spacing: .small) {
            ForEach([25, 50, 75, 100], id: \.self) { percentage in
                Button {
                    model.onSelectPercentage(percentage)
                } label: {
                    Text(percentage.formatted(.percent.scale(1)))
                        .font(.wowPay.rate.weight(.bold))
                        .foregroundStyle(Colors.WowPay.brand)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Spacing.space10)
                        .background(Colors.WowPay.secondaryButton, in: RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner))
                }
            }
        }
        .buttonStyle(.plain)
    }

    private var feeCard: some View {
        HStack(spacing: Spacing.space12) {
            Image(systemName: "cylinder.split.1x2")
                .font(.wowPay.cardTitle)
                .foregroundStyle(Colors.WowPay.brand)
            VStack(alignment: .leading, spacing: .tiny) {
                HStack(spacing: .tiny) {
                    Text(Localized.wowpayNetworkFee)
                        .font(.wowPay.label)
                    Button {
                        isPresentingFeeInfo = true
                    } label: {
                        Image(systemName: "questionmark.circle")
                            .foregroundStyle(Colors.WowPay.brand)
                    }
                    .accessibilityLabel(Localized.Info.NetworkFee.title)
                }
                if let error = model.feeError {
                    HStack(spacing: .small) {
                        Text(error)
                            .foregroundStyle(Colors.red)
                        Button(Localized.Common.tryAgain, action: model.onSelectRetryFee)
                            .foregroundStyle(Colors.WowPay.brand)
                    }
                    .font(.wowPay.caption)
                } else if !model.hasFeeQuote {
                    Text(Localized.wowpaySendFeePending)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                } else {
                    Text("\(model.feeEquivalent) (\(Localized.wowpaySendFeeEstimated))")
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
            }
            Spacer(minLength: .small)
            Text(model.feeAmount)
                .font(.wowPay.label)
                .foregroundStyle(Colors.WowPay.heading)
                .lineLimit(1)
                .minimumScaleFactor(0.75)
        }
        .padding(Spacing.wowPay.card)
        .background(Colors.WowPay.pale, in: RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner))
        .wowPayCard(padding: Spacing.wowPay.card)
    }

    private var contactPicker: some View {
        ContactRecipientPicker(
            recipients: model.contactRecipients,
            onSelect: model.onSelectContact,
            onClose: { model.isPresentingContacts = false },
            walletRecipients: model.walletRecipients,
            onSelectWallet: model.onSelectWallet,
            walletRecipientsState: model.walletRecipientsState,
            onRetryWallets: { Task { await model.refreshWalletRecipients(wallets: model.walletsQuery.value) } },
        )
    }

    private func onSelectConfirm() {
        focusedField = nil
        model.onSelectConfirm()
    }
}
