// Copyright (c). Gem Wallet. All rights reserved.

import Assets
import BsvMultisig
import Components
import Foundation
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import QRScanner
import Store
import SwiftUI
import Transfer
import WalletTab

struct AssetNavigationView: View {
    @Environment(\.viewModelFactory) private var viewModelFactory
    @Environment(\.navigationPresenter) private var presenter
    @Environment(\.navigationState) private var navigationState
    @State private var model: AssetSceneViewModel
    @State private var paymailModel: WowPayPaymailViewModel?
    @State private var isPresentingPaymail = false
    @State private var isPresentingSendPaymail = false
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @State private var send = WowPaySendPresentation()
    private let walletHeader: AnyView?

    init(model: AssetSceneViewModel, initialTab _: Scenes.Asset.InitialTab? = nil, multisigModel _: BsvMultisigViewModel?, walletHeader: AnyView? = nil) {
        self.walletHeader = walletHeader
        model.selectedTab = .overview
        _model = State(initialValue: model)
    }

    var body: some View {
        AssetScene(model: model, paymailHandle: paymailModel?.activeHandle, paymailStatus: paymailModel?.receiveTitle ?? Localized.Common.loading, onPaymail: { isPresentingPaymail = true }, onSend: onSelectSend, onReceive: onSelectReceive, walletHeader: walletHeader) {
            EmptyView()
        }
        .navigationDestination(isPresented: $isPresentingPaymail) {
            WowPayPaymailScene(model: viewModelFactory.paymailScene(walletId: model.walletModel.wallet.id))
        }
        .task {
            guard model.showsPaymail else { return }
            let paymailModel = paymailModel ?? viewModelFactory.paymailScene(walletId: model.walletModel.wallet.id)
            self.paymailModel = paymailModel
            await paymailModel.load()
        }
        .onDisappear {
            paymailModel?.cancel()
        }
        .bindQuery(model.assetQuery, model.bannersQuery, model.transactionsQuery)
        .toast(message: $model.isPresentingToastMessage)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                send.dismiss()
            }
        }
        .navigationDestination(for: WowPaySendDestination.self) { destination in
            if send.isCurrent(destination.id), let sendModel = send.model {
                @Bindable var formModel = sendModel
                @Bindable var confirmation = send.confirmation
                WowPaySendScene(model: sendModel)
                    .sheet(isPresented: $formModel.isPresentingScanner) {
                        ScanQRCodeNavigationStack(scanType: .address, action: sendModel.onHandleScan)
                    }
                    .navigationDestination(isPresented: $isPresentingSendPaymail) {
                        WowPayPaymailScene(model: viewModelFactory.paymailScene(walletId: model.walletModel.wallet.id))
                    }
                    .navigationDestination(isPresented: $confirmation.isPresented) {
                        if let model = send.confirmation.model {
                            ConfirmTransferNavigationView(model: model)
                                .id(model.id)
                        }
                    }
            }
        }
        .sheet(item: $model.isPresentingAssetSheet) {
            switch $0 {
            case let .info(type):
                InfoSheetScene(type: type)
            case let .transfer(data):
                ConfirmTransferNavigationStack(
                    wallet: model.walletModel.wallet,
                    transferData: data,
                    onComplete: model.onTransferComplete,
                )
            case .share:
                ShareSheet(activityItems: [model.shareAssetUrl.absoluteString])
            case let .url(url):
                SFSafariView(url: url)
            }
        }
    }

    private func openColdWallet(entry: Scenes.BsvColdWallet.Entry) {
        guard let navigationOwner else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
            return
        }
        navigationOwner.append(Scenes.BsvColdWallet(walletId: model.walletModel.wallet.id.id, entry: entry))
    }

    private func onSelectReceive(_ assetData: AssetData) {
        if assetData.asset.id == Chain.bitcoinSv.asset.id {
            do {
                switch try viewModelFactory.coldWalletRole(for: model.walletModel.wallet) {
                case .observer:
                    openColdWallet(entry: .receive)
                    return
                case .signer:
                    model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
                    return
                case .none:
                    break
                }
            } catch {
                model.isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "info.circle")
                return
            }
        }
        model.onSelectReceive()
    }

    private func onSelectSend(_ assetData: AssetData) {
        let wallet = model.walletModel.wallet
        if assetData.asset.id == Chain.bitcoinSv.asset.id {
            do {
                switch try viewModelFactory.coldWalletRole(for: wallet) {
                case .observer:
                    openColdWallet(entry: .send)
                    return
                case .signer:
                    model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
                    return
                case .none:
                    break
                }
            } catch {
                model.isPresentingToastMessage = ToastMessage(title: Localized.coldWalletStartupStorageError, image: "info.circle")
                return
            }
        }
        guard wallet.canSign,
              assetData.account.chain == assetData.asset.chain,
              !assetData.account.address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              wallet.accounts.contains(where: {
                  $0.chain == assetData.account.chain && $0.address == assetData.account.address
              })
        else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
            return
        }
        guard let navigationOwner else {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
            return
        }
        do {
            let id = UUID()
            let senderId = wallet.id
            let tab = navigationState.selectedTab
            let selection = navigationState.walletPresentation
            let sendModel = try viewModelFactory.wowPaySendScene(
                wallet: wallet,
                assetData: assetData,
                onTransferAction: { data in
                    guard send.isCurrent(id) else { return }
                    send.confirmation.present(model: viewModelFactory.confirmTransferScene(wallet: model.walletModel.wallet, data: data, onComplete: { send.dismiss(id: id) }))
                },
                onManagePaymail: {
                    guard send.isCurrent(id) else { return }
                    isPresentingSendPaymail = true
                },
                onPublicFunding: { source in
                    guard send.isCurrent(id) else {
                        source.cancelPaymailPreparation()
                        return
                    }
                    send.dismiss(id: id)
                    let parentPath = navigationOwner.path
                    presenter.requestMultisig(.publicFunding(source), navigationMode: .push, isActive: {
                        viewModelFactory.walletSessionService.currentWalletId == senderId && navigationState.selectedTab == tab
                            && navigationState.walletPresentation == selection && navigationOwner.path == parentPath && scenePhase != .background
                    })
                },
            )
            let exitObserver = navigationState.observeTabExit(tab: tab) {
                send.dismiss(id: id)
            }
            send.present(id: id, model: sendModel, navigation: navigationOwner, onRetire: {
                isPresentingSendPaymail = false
                navigationState.removeTabExitObserver(exitObserver)
            })
        } catch {
            model.isPresentingToastMessage = ToastMessage(title: Localized.wowpayUnavailable, image: "info.circle")
        }
    }
}
