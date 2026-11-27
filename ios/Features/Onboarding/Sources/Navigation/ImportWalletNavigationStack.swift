// Copyright (c). Gem Wallet. All rights reserved.

public import class Components.NavigationPathState
import BsvHomeCoreServices
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

public struct ImportWalletNavigationStack: View {
    public enum Entry: Sendable {
        case methods
        case phrase
    }

    @State private var model: ImportWalletViewModel
    @State private var navigation: WalletOnboardingNavigation
    @State private var isShowingMethods: Bool
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    private let coldWalletContent: (@MainActor () -> AnyView)?
    private let coldWalletDestination: (@MainActor (NavigationPathState) -> AnyView)?
    private let connectColdWalletDestination: (@MainActor (NavigationPathState) -> AnyView)?
    private let multisigWalletContent: (@MainActor (Binding<NavigationPath>) -> AnyView)?
    private let entry: Entry
    private let navigationContent: (@MainActor (AnyView, NavigationPathState) -> AnyView)?

    public init(
        model: ImportWalletViewModel,
        coldWalletContent: (@MainActor () -> AnyView)? = nil,
        coldWalletDestination: (@MainActor (NavigationPathState) -> AnyView)? = nil,
        connectColdWalletDestination: (@MainActor (NavigationPathState) -> AnyView)? = nil,
        multisigWalletContent: (@MainActor (Binding<NavigationPath>) -> AnyView)? = nil,
        onMultisigDismiss: (@MainActor () -> Void)? = nil,
        onColdWalletDismiss: (@MainActor () -> Void)? = nil,
        entry: Entry = .methods,
        navigationContent: (@MainActor (AnyView, NavigationPathState) -> AnyView)? = nil,
    ) {
        _model = State(initialValue: model)
        _navigation = State(initialValue: WalletOnboardingNavigation(onMultisigDismiss: onMultisigDismiss, onColdWalletDismiss: onColdWalletDismiss))
        _isShowingMethods = State(initialValue: model.isAcceptTermsCompleted)
        self.coldWalletContent = coldWalletContent
        self.coldWalletDestination = coldWalletDestination
        self.connectColdWalletDestination = connectColdWalletDestination
        self.multisigWalletContent = multisigWalletContent
        self.entry = entry
        self.navigationContent = navigationContent
    }

    public var body: some View {
        NavigationStack(path: navigation.binding) {
            let content = AnyView(
                rootScene
                    .toolbar {
                        ToolbarItem(placement: .topBarLeading) {
                            Button(Localized.Common.cancel, systemImage: SystemImage.xmark) {
                                navigation.cancel()
                                model.cancel()
                                dismiss()
                            }
                            .labelStyle(.iconOnly)
                        }
                    }
                    .navigationBarTitleDisplayMode(.inline)
                    .navigationDestination(for: ImportWalletInputKind.self) { kind in
                        ImportWalletInputScene(
                            kind: kind,
                            importModel: model,
                            acceptedTermsThisSession: isShowingMethods,
                            onNewWallet: onNewWallet,
                            onExistingWallet: { model.onComplete?() },
                        )
                    }
                    .navigationDestination(for: Scenes.WalletProfile.self) { scene in
                        SetupWalletScene(model: model.setupWalletModel(wallet: scene.wallet, onComplete: onSetupWalletComplete))
                            .navigationBarBackButtonHidden()
                            .interactiveDismissDisabled()
                    }
                    .navigationDestination(for: WalletOnboardingNavigation.Destination.self) { destination in
                        switch destination {
                        case .multisig:
                            if let multisigWalletContent {
                                multisigWalletContent(navigation.binding)
                            }
                        case .coldRecover:
                            if let coldWalletDestination {
                                coldWalletDestination(navigation.navigation)
                            } else if let coldWalletContent {
                                coldWalletContent()
                            }
                        case .coldConnect:
                            if let connectColdWalletDestination {
                                connectColdWalletDestination(navigation.navigation)
                            }
                        case .coldCreate:
                            EmptyView()
                        }
                    }
                    .sheet(item: $model.isPresentingSelectImageWallet) { wallet in
                        NavigationStack {
                            WalletImageScene(model: model.walletImageModel(wallet: wallet))
                                .toolbarDismissItem(type: .close, placement: .topBarLeading)
                        }
                    },
            )
            if let navigationContent {
                navigationContent(content, navigation.navigation)
            } else {
                content
            }
        }
        .environment(navigation.navigation)
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                navigation.cancel()
                model.cancel()
                dismiss()
            }
        }
    }

    @ViewBuilder
    private var rootScene: some View {
        if isShowingMethods {
            switch entry {
            case .methods:
                ImportWalletMethodScene(
                    onPhrase: { navigation.append(ImportWalletInputKind.phrase) },
                    onPrivateKey: { navigation.append(ImportWalletInputKind.privateKey) },
                    onWatch: { navigation.append(ImportWalletInputKind.address) },
                    onColdWallet: onSelectColdWallet,
                    onConnectColdWallet: onSelectConnectColdWallet,
                    onJoinMultisig: onSelectMultisigWallet,
                )
            case .phrase:
                ImportWalletInputScene(kind: .phrase, importModel: model, acceptedTermsThisSession: isShowingMethods, onNewWallet: onNewWallet, onExistingWallet: { model.onComplete?() })
            }
        } else {
            AcceptTermsScene(model: AcceptTermsViewModel(onNext: { isShowingMethods = true }))
        }
    }

    private var onSelectColdWallet: (() -> Void)? {
        guard coldWalletDestination != nil || coldWalletContent != nil else { return nil }
        return { navigation.openColdRecover() }
    }

    private var onSelectConnectColdWallet: (() -> Void)? {
        guard connectColdWalletDestination != nil else { return nil }
        return { navigation.openColdConnect() }
    }

    private var onSelectMultisigWallet: (() -> Void)? {
        guard multisigWalletContent != nil else { return nil }
        return { navigation.openMultisig() }
    }
}

// MARK: - Actions

extension ImportWalletNavigationStack {
    func onNewWallet(_ wallet: Wallet) {
        navigation.append(Scenes.WalletProfile(wallet: wallet))
    }

    func onSetupWalletComplete(_: Wallet) {
        model.onComplete?()
    }
}
