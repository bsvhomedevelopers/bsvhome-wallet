// Copyright (c). Gem Wallet. All rights reserved.

import BsvColdWallet
import Components
import Foundation
import ManageWallets
import Onboarding
import Primitives
import Style
import SwiftUI

struct WalletsNavigationStack: View {
    @Environment(\.viewModelFactory) private var viewModelFactory
    @Environment(\.dismiss) private var dismiss
    @Environment(RootSceneViewModel.self) private var rootModel

    @State private var navigation = NavigationPathState()

    @State private var isPresentingCreateWalletSheet = false
    @State private var isPresentingImportWalletSheet = false
    @State private var createModel: CreateWalletModel?
    @State private var importModel: ImportWalletViewModel?

    private func coldWalletNavigation(_ content: AnyView, navigation: NavigationPathState) -> AnyView {
        AnyView(content.bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigation)?.model, navigation: navigation, onInitialPageBack: rootModel.coldWalletContext(in: navigation)?.onInitialPageBack))
    }

    var body: some View {
        NavigationStack(path: navigation.binding) {
            WalletsScene(
                model: viewModelFactory.walletsScene(
                    navigationPath: navigation.binding,
                    isPresentingCreateWalletSheet: createBinding,
                    isPresentingImportWalletSheet: importBinding,
                ),
            )
            .navigationDestination(for: Scenes.WalletDetail.self) {
                WalletDetailScene(model: viewModelFactory.walletDetailScene(navigationPath: navigation.binding, wallet: $0.wallet, onOpenPairing: { wallet in
                    navigation.append(Scenes.BsvColdWallet(walletId: wallet.id.id, entry: .pairing))
                }, onOpenSigning: { wallet in
                    navigation.append(Scenes.BsvColdWallet(walletId: wallet.id.id, entry: .signing))
                }))
            }
            .bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigation)?.model, navigation: navigation, onScanRecipient: rootModel.coldWalletContext(in: navigation)?.onScanRecipient, onInitialPageBack: rootModel.coldWalletContext(in: navigation)?.onInitialPageBack)
            .navigationDestination(for: Scenes.BsvColdWallet.self) { ColdWalletDestination(scene: $0, navigation: navigation) }
            .navigationDestination(for: Scenes.WalletSelectImage.self) {
                WalletImageScene(model: viewModelFactory.walletImageScene(wallet: $0.wallet))
            }
            .sheet(isPresented: createBinding) {
                if let createModel {
                    CreateWalletNavigationStack(model: createModel, coldWalletDestination: { path in
                        AnyView(ColdWalletOnboardingDestination(entry: .create, owner: ObjectIdentifier(createModel), navigation: path, onComplete: completeOnboarding))
                    }, onColdWalletDismiss: { rootModel.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(createModel)) }, navigationContent: coldWalletNavigation)
                }
            }
            .sheet(isPresented: importBinding) {
                if let importModel {
                    ImportWalletNavigationStack(model: importModel, coldWalletDestination: { path in
                        AnyView(ColdWalletOnboardingDestination(entry: .recover, owner: ObjectIdentifier(importModel), navigation: path, onComplete: completeOnboarding))
                    }, connectColdWalletDestination: { path in
                        AnyView(ColdWalletOnboardingDestination(entry: .connect, owner: ObjectIdentifier(importModel), navigation: path, onComplete: completeOnboarding))
                    }, onColdWalletDismiss: { rootModel.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(importModel)) }, navigationContent: coldWalletNavigation)
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("", systemImage: SystemImage.xmark) {
                        dismiss()
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
        }
        .onDisappear { rootModel.cancelColdWalletContext(navigation: navigation) }
    }

    private var createBinding: Binding<Bool> {
        Binding(get: { isPresentingCreateWalletSheet }, set: {
            if $0, createModel == nil {
                createModel = viewModelFactory.createWalletScene(onComplete: completeOnboarding)
            } else if !$0 {
                if let createModel {
                    rootModel.cancelColdWalletContext(owner: ObjectIdentifier(createModel))
                }
                createModel?.cancel()
                createModel = nil
            }
            isPresentingCreateWalletSheet = $0
        })
    }

    private var importBinding: Binding<Bool> {
        Binding(get: { isPresentingImportWalletSheet }, set: {
            if $0, importModel == nil {
                importModel = viewModelFactory.importWalletScene(onComplete: completeOnboarding)
            } else if !$0 {
                if let importModel {
                    rootModel.cancelColdWalletContext(owner: ObjectIdentifier(importModel))
                }
                importModel?.cancel()
                importModel = nil
            }
            isPresentingImportWalletSheet = $0
        })
    }

    private func completeOnboarding() {
        createBinding.wrappedValue = false
        importBinding.wrappedValue = false
        dismiss()
    }
}
