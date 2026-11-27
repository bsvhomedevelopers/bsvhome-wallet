// Copyright (c). Gem Wallet. All rights reserved.

public import class Components.NavigationPathState
import Localization
import PrimitivesComponents
import SwiftUI

public struct CreateWalletNavigationStack: View {
    public enum Entry: Sendable {
        case methods
        case securityReminder
    }

    @State private var flow: CreateWalletFlow
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var navigation: WalletOnboardingNavigation
    @State private var isShowingMethods: Bool
    private let coldWalletContent: (@MainActor () -> AnyView)?
    private let coldWalletDestination: (@MainActor (NavigationPathState) -> AnyView)?
    private let multisigWalletContent: (@MainActor (Binding<NavigationPath>) -> AnyView)?
    private let entry: Entry
    private let navigationContent: (@MainActor (AnyView, NavigationPathState) -> AnyView)?

    public init(
        model: CreateWalletModel,
        coldWalletContent: (@MainActor () -> AnyView)? = nil,
        coldWalletDestination: (@MainActor (NavigationPathState) -> AnyView)? = nil,
        multisigWalletContent: (@MainActor (Binding<NavigationPath>) -> AnyView)? = nil,
        onMultisigDismiss: (@MainActor () -> Void)? = nil,
        onColdWalletDismiss: (@MainActor () -> Void)? = nil,
        entry: Entry = .methods,
        navigationContent: (@MainActor (AnyView, NavigationPathState) -> AnyView)? = nil,
    ) {
        let navigation = WalletOnboardingNavigation(onMultisigDismiss: onMultisigDismiss, onColdWalletDismiss: onColdWalletDismiss)
        _navigation = State(initialValue: navigation)
        _flow = State(initialValue: CreateWalletFlow(model: model) { step in
            navigation.appendDistinct(step)
        })
        _isShowingMethods = State(initialValue: model.isAcceptTermsCompleted)
        self.coldWalletContent = coldWalletContent
        self.coldWalletDestination = coldWalletDestination
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
                            Button(Localized.Common.cancel) {
                                navigation.cancel()
                                flow.model.cancel()
                                dismiss()
                            }
                        }
                    }
                    .navigationDestination(for: CreateWalletFlow.Step.self) { step in
                        CreateWalletFlowScene(flow: flow, step: step)
                    }
                    .navigationDestination(for: WalletOnboardingNavigation.Destination.self) { destination in
                        switch destination {
                        case .multisig:
                            if let multisigWalletContent {
                                multisigWalletContent(navigation.binding)
                            }
                        case .coldCreate:
                            if let coldWalletDestination {
                                coldWalletDestination(navigation.navigation)
                            } else if let coldWalletContent {
                                coldWalletContent()
                            }
                        case .coldRecover, .coldConnect:
                            EmptyView()
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
                flow.model.cancel()
                dismiss()
            }
        }
    }

    @ViewBuilder
    private var rootScene: some View {
        if isShowingMethods {
            switch entry {
            case .methods:
                CreateWalletMethodScene(
                    onCreateOnline: { navigation.appendDistinct(CreateWalletFlow.Step.securityReminder) },
                    onCreateCold: onSelectColdWallet,
                    onCreateMultisig: onSelectMultisigWallet,
                )
            case .securityReminder:
                CreateWalletFlowScene(flow: flow, step: .securityReminder)
            }
        } else {
            AcceptTermsScene(model: AcceptTermsViewModel(onNext: { isShowingMethods = true }))
        }
    }

    private var onSelectColdWallet: (() -> Void)? {
        guard coldWalletDestination != nil || coldWalletContent != nil else { return nil }
        return { navigation.openColdCreate() }
    }

    private var onSelectMultisigWallet: (() -> Void)? {
        guard multisigWalletContent != nil else { return nil }
        return { navigation.openMultisig() }
    }
}
