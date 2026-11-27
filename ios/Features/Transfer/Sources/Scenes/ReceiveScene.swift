// Copyright (c). Gem Wallet. All rights reserved.

import Components
import BsvHomeCorePrimitives
import Localization
import PrimitivesComponents
import Style
import SwiftUI

public struct ReceiveScene: View {
    @State private var model: ReceiveViewModel
    @Environment(\.connectionStatus) private var connectionStatus
    @Environment(\.verticalSizeClass) private var verticalSizeClass
    private let navigationTitle: String

    public init(model: ReceiveViewModel, navigationTitle: String = "") {
        _model = State(initialValue: model)
        self.navigationTitle = navigationTitle
    }

    public var body: some View {
        ScrollView {
            receiveContent
                .padding(.horizontal, Spacing.small)
                .padding(Spacing.wowPay.page)
        }
        .wowPayBackground(.solid)
        .navigationTitle(navigationTitle)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(Localized.Common.share, systemImage: "square.and.arrow.up", action: model.onShareSheet)
                    .disabled(!model.canShareAddress)
                    .accessibilityIdentifier("receiveShare")
            }
        }
        .sheet(item: $model.isPresentingSheet) { type in
            switch type {
            case .share:
                ShareSheet(activityItems: model.activityItems(qrImage: model.renderedImage))
            case .networkSelector:
                SelectableListNavigationStack(
                    model: model.networkSelectorModel,
                    onFinishSelection: { model.onFinishNetworkSelection($0) },
                    listContent: {
                        ChainView(
                            model: ChainViewModel(
                                chain: $0.assetId.chain,
                                assetType: $0.assetId.assetType,
                            ),
                        )
                    },
                )
                .presentationDetents([.large])
            case .copy:
                EmptyView()
            }
        }
        .copyToast(
            model: model.copyModel,
            isPresenting: $model.isPresentingCopyToast,
        )
        .alertSheet($model.isPresentingAlertMessage)
        .task {
            await model.onLoadImage()
        }
        .task(id: verticalSizeClass) {
            await model.onChangeSize(isCompact: verticalSizeClass == .compact)
        }
        .taskOnce(model.onTaskOnce)
        .refreshableTimer(every: connectionStatus.refreshInterval(for: .wallet)) { _ in
            await model.refreshBsvAddress()
        }
        .onDisappear(perform: model.onCancelBsvScan)
    }
}

extension ReceiveScene {
    private var receiveContent: some View {
        VStack(spacing: Spacing.wowPay.page) {
            HStack(spacing: Spacing.wowPay.card) {
                AssetImageView(assetImage: model.assetModel.assetImage, size: Sizing.wowPay.detailAssetIcon)
                Text(model.assetModel.name)
                    .font(.wowPay.onboardingTitle)
                    .accessibilityAddTraits(.isHeader)
            }

            Button(action: model.onCopyAddress) {
                Group {
                    if let image = model.renderedImage {
                        qrCodeView(image: image)
                    } else if model.receiveState.isLoading || model.receiveState.isNoData {
                        LoadingView()
                    } else {
                        Text(Localized.Common.notAvailable)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                }
                .frame(width: model.qrSize, height: model.qrSize)
                .background(Colors.whiteSolid, in: RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner))
                .overlay {
                    RoundedRectangle(cornerRadius: Sizing.wowPay.rowCorner)
                        .strokeBorder(Colors.WowPay.border, lineWidth: .space1)
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.copyTitle)
            .disabled(model.address.isEmpty)

            if case let .error(error) = model.receiveState {
                StateEmptyView(title: error.isCancelled ? Localized.Errors.cancelled : Localized.Errors.errorOccurred) {
                    Button(Localized.Common.tryAgain) {
                        model.onRetryImage()
                    }
                    .buttonStyle(.wowPaySecondary())
                }
            } else if model.isBsv, model.address.isEmpty, !model.receiveState.isLoading {
                Text(Localized.wowpayBsvAddressAllocationPaused)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
            }
            if model.showsBsvRecovery, let snapshot = model.bsvAddressSnapshot {
                Text(snapshot.discovery.statusTitle)
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                if model.isRefreshing {
                    Button(Localized.Common.cancel, action: model.onCancelBsvScan)
                        .buttonStyle(.wowPaySecondary())
                } else {
                    Button(Localized.wowpayBsvContinueScan) { model.onContinueBsvScan() }
                        .buttonStyle(.wowPaySecondary())
                        .disabled(model.receiveState.isLoading)
                }
            }
            VStack(spacing: Spacing.space12) {
                Button(action: model.onCopyAddress) {
                    Text(model.address.preventingHyphenation)
                        .font(.wowPay.caption)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity)
                        .padding(Spacing.space12)
                        .background(Colors.WowPay.addressSurface, in: RoundedRectangle(cornerRadius: Spacing.space12))
                        .accessibilityIdentifier(model.address)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("receiveAddress")
                .disabled(model.address.isEmpty)

                Text(.init(model.warningMessage))
                    .font(.wowPay.caption)
                    .foregroundStyle(Colors.WowPay.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .multilineTextAlignment(.leading)

                if model.showNetworkSelector {
                    Button(action: model.onSelectNetwork) {
                        HStack {
                            ChainView(
                                model: ChainViewModel(
                                    chain: model.assetModel.asset.chain,
                                    assetType: model.assetModel.asset.type,
                                ),
                            )
                            Spacer()
                            Images.System.chevronRight
                        }
                    }
                    .buttonStyle(.wowPaySecondary())
                }
            }

            Button(model.copyTitle, action: model.onCopyAddress)
                .buttonStyle(.wowPay())
                .disabled(model.address.isEmpty)
                .accessibilityIdentifier("receiveCopy")
                .padding(.top, Spacing.wowPay.section)
        }
        .multilineTextAlignment(.center)
        .padding(.top, Spacing.space24)
        .padding(.bottom, Spacing.space32)
    }

    private func qrCodeView(image: UIImage) -> some View {
        Image(uiImage: image)
            .resizable()
            .interpolation(.none)
            .scaledToFit()
            .frame(width: model.qrSize, height: model.qrSize)
    }
}
