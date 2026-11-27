// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import Primitives
import PrimitivesComponents
import Store
import Style
import Swap
import SwiftUI

public struct ConfirmTransferScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var executionLifetime: ConfirmExecutionLifetime?
    @Bindable var model: ConfirmTransferSceneViewModel

    public init(model: ConfirmTransferSceneViewModel) {
        self.model = model
    }

    public var body: some View {
        ListSectionView(
            provider: model,
            content: { item in
                content(for: item)
                    .listRowInsets(EdgeInsets(
                        top: Spacing.wowPay.assetRowVertical,
                        leading: Spacing.wowPay.card,
                        bottom: Spacing.wowPay.assetRowVertical,
                        trailing: Spacing.wowPay.card,
                    ))
                    .listRowSeparatorTint(Colors.WowPay.border)
            },
        )
        .listStyle(.insetGrouped)
        .contentMargins(.horizontal, Spacing.wowPay.page, for: .scrollContent)
        .contentMargins(.top, Spacing.wowPay.card, for: .scrollContent)
        .listSectionSpacing(.custom(.space12))
        .safeAreaButton {
            StateButton(model.confirmButtonModel, type: model.confirmButtonModel.type.wowPay)
        }
        .frame(maxWidth: .infinity)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .task(id: model.preloadSelection) {
            await model.load()
        }
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .activityIndicator(isLoading: model.isConfirming, message: model.progressMessage)
        .alertSheet($model.isPresentingAlertMessage)
        .onAppear {
            if executionLifetime == nil {
                executionLifetime = model.makeExecutionLifetime()
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                model.cancelExecution()
            }
        }
        .sheet(item: passwordPresentation) { password in
            NavigationStack { TransferPasswordScene(model: password) }
                .presentationDetents([.medium, .large])
        }
    }
}

// MARK: - UI Components

extension ConfirmTransferScene {
    @ViewBuilder
    private func content(for itemModel: ConfirmTransferItemModel) -> some View {
        switch itemModel {
        case let .header(model):
            TransactionHeaderListItemView(
                headerType: model.headerType,
                showClearHeader: model.showClearHeader,
                style: .wowPay,
            )
            .isVisible(self.model.isHeaderVisible)
        case let .app(model):
            ListItemImageView(model: model, style: .wowPay)
                .contextMenu(
                    .url(title: self.model.websiteTitle, onOpen: self.model.onSelectOpenWebsiteURL),
                )
        case let .sender(model):
            ListItemImageView(model: model, style: .wowPay)
                .explorerContext(self.model.senderExplorerContext)
        case let .recipient(model):
            AddressListItemView(model: model, style: .wowPay)
        case let .network(model):
            ListItemImageView(model: model, style: .wowPay)
        case let .additionalDetail(model):
            ListItemView(model: model, style: .wowPay)
        case let .memo(model):
            ListItemView(model: model, style: .wowPay)
                .contextMenu(model.subtitle.map { [.copy(value: $0)] } ?? [])
        case let .swapDetails(model):
            NavigationCustomLink(
                with: SwapDetailsListView(model: model),
                action: { self.model.onSelectSwapDetails() },
            )
        case let .perpetualDetails(model):
            NavigationCustomLink(
                with: ListItemView(model: model.listItemModel, style: .wowPay),
                action: { self.model.onSelectPerpetualDetails(model) },
            )
        case let .perpetualModifyPosition(model):
            if let listItemModel = model.listItemModel {
                ListItemView(model: listItemModel, style: .wowPay)
            }
        case let .networkFee(model, selectable):
            if selectable {
                NavigationCustomLink(
                    with: ListItemView(model: model, style: .wowPay),
                    action: self.model.onSelectFeePicker,
                )
                .listRowBackground(Colors.WowPay.pale)
            } else {
                ListItemView(model: model, style: .wowPay)
                    .listRowBackground(Colors.WowPay.pale)
            }
        case let .warnings(warnings):
            SimulationWarningsContent(warnings: warnings)
        case let .balanceChange(model):
            ListItemView(
                title: TextValue(text: model.assetTitle, style: .wowPay.rowTitle, lineLimit: 1, truncationMode: .tail),
                subtitle: model.amount,
                imageStyle: .list(assetImage: model.assetImage, cornerRadiusType: .rounded),
            )
        case let .payload(fields):
            Group {
                SimulationPayloadFieldsContent(
                    fields: fields,
                    fieldViewModel: self.model.payloadModel.fieldViewModel(for:),
                    contextMenuItems: self.model.contextMenuItems(for:),
                )

                if self.model.payloadModel.hasDetails {
                    NavigationCustomLink(
                        with: ListItemView(title: Localized.Common.details, titleStyle: .wowPay.rowTitle),
                        action: self.model.onSelectPayloadDetails,
                    )
                }
            }
        case let .error(title, error, onInfoAction):
            ListItemErrorView(
                errorTitle: title,
                error: error,
                infoAction: onInfoAction,
            )
        case .empty:
            EmptyView()
        }
    }

    private var passwordPresentation: Binding<TransferPasswordViewModel?> {
        let authorization = model.passwordAuthorization
        let requestId = authorization?.requestId
        return Binding(
            get: { authorization?.presentation },
            set: { replacement in
                if replacement == nil, let requestId {
                    authorization?.cancel(requestId: requestId)
                }
            },
        )
    }
}
