import BsvMultisig
import Components
import Localization
import Onboarding
import QRScanner
import Style
import SwiftUI

struct CollectiveOnboardingScene: View {
    private enum Destination: Hashable { case join, restore }

    @Environment(RootSceneViewModel.self) private var rootModel
    @Environment(NavigationPathState.self) private var navigationOwner: NavigationPathState?
    @Environment(\.scenePhase) private var scenePhase
    @Bindable var model: CollectiveOnboardingModel
    @Binding var path: NavigationPath
    var header: AnyView?
    var listContext: BsvMultisigListContext?
    @State private var rootDepth: Int?

    var body: some View {
        rootContent
            .id(model.scopeId)
            .bsvMultisigNavigation(model: model.featureModel, navigationPath: $path, identityContext: identityContext(for: initialRoute), onScanInvitation: scanner, identityContextForRoute: identityContext)
            .environment(\.bsvMultisigRecipientScanner, recipientScanner)
            .navigationDestination(for: Destination.self) { destination in
                switch destination {
                case .join: featureContent(.join)
                case .restore: recoveryChoices
                }
            }
            .sheet(item: modalBinding) { modal in
                switch modal.content {
                case let .create(model): CreateWalletNavigationStack(model: model, entry: .securityReminder)
                case let .recovery(model): ImportWalletNavigationStack(model: model, entry: .phrase)
                }
            }
            .sheet(item: model.scanner.binding) { scan in
                ScanQRCodeNavigationStack(scanType: scan.scanType, action: { value in model.scanner.complete(id: scan.id, value: value) })
                    .orientationLock(.portrait)
            }
            .bsvMultisigPassword(authorization: model.authorization)
            .onAppear {
                if rootDepth == nil {
                    rootDepth = path.count
                }
                rootModel.registerCollectiveOnboarding(model)
                model.start(afterUnlock: waitUntilUnlocked)
            }
            .onChange(of: model.entry) { _, entry in
                if entry == .list, let rootDepth, path.count > rootDepth {
                    path.removeLast(path.count - rootDepth)
                }
            }
            .onChange(of: model.identityModal?.id) { _, current in
                if current == nil {
                    model.refresh(afterUnlock: waitUntilUnlocked)
                }
            }
            .onChange(of: scenePhase) { _, phase in
                if phase == .background {
                    model.cancel()
                } else if phase == .active {
                    model.start(afterUnlock: waitUntilUnlocked)
                }
            }
    }

    @ViewBuilder
    private var rootContent: some View {
        switch model.entry {
        case .create:
            featureContent(.create)
                .accessibilityIdentifier("collectiveCreateRoot")
        case let .join(invitation):
            featureContent(.join, initialInvitation: invitation)
        case .importChoice:
            choices(title: Localized.multisigImportTitle) {
                NavigationLink(value: Destination.join) {
                    WowPaySelectionRow(title: Localized.multisigJoin, subtitle: Localized.multisigJoinNotice, image: Image("wowpay-wallet"))
                }
                .accessibilityIdentifier("collectiveImportJoin")
                NavigationLink(value: Destination.restore) {
                    WowPaySelectionRow(title: Localized.multisigRestoreIdentity, subtitle: Localized.wowpayImportMethodPhraseSubtitle, image: Image("wowpay-shield-check"))
                }
                .accessibilityIdentifier("collectiveImportRestore")
            }
            .accessibilityIdentifier("collectiveImportChoices")
        case .list:
            if let header {
                ScrollView {
                    VStack(spacing: Spacing.wowPay.card) {
                        header
                        if let featureModel = model.featureModel {
                            BsvMultisigListView(model: featureModel, context: listContext)
                                .accessibilityIdentifier("collectiveWalletList")
                        } else {
                            missingIdentity
                        }
                    }
                    .padding(Spacing.wowPay.page)
                }
                .wowPayBackground()
                .accessibilityIdentifier("collectiveOverview")
                .toolbar(.hidden, for: .navigationBar)
            } else {
                featureContent(.list)
                    .accessibilityIdentifier("collectiveWalletList")
            }
        }
    }

    private var recoveryChoices: some View {
        choices(title: Localized.multisigRestoreIdentity) {
            Button { model.presentRecovery(dedicated: false) } label: {
                WowPaySelectionRow(title: Localized.multisigRestoreExisting, subtitle: Localized.wowpayImportMethodPhraseSubtitle, image: Image("wowpay-wallet"))
            }
            .accessibilityIdentifier("collectiveRestoreExisting")
            Button { model.presentRecovery(dedicated: true) } label: {
                WowPaySelectionRow(title: Localized.multisigRestoreDedicated, subtitle: Localized.wowpayImportMethodPhraseSubtitle, image: Image("wowpay-shield-check"))
            }
            .accessibilityIdentifier("collectiveRestoreDedicated")
        }
        .accessibilityIdentifier("collectiveRestoreChoices")
    }

    private func choices(title: String, @ViewBuilder content: () -> some View) -> some View {
        ScrollView {
            VStack(spacing: Spacing.wowPay.card, content: content).padding(Spacing.wowPay.page)
        }
        .buttonStyle(.plain)
        .wowPayBackground()
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
    }

    @ViewBuilder
    private func featureContent(_ route: BsvMultisigRoute, initialInvitation: String? = nil) -> some View {
        if let featureModel = model.featureModel {
            BsvMultisigDestination(model: featureModel, route: route, initialInvitation: initialInvitation)
                .environment(\.bsvMultisigIdentityContext, identityContext(for: route))
                .environment(\.bsvMultisigInvitationScanner, scanner)
                .environment(\.bsvMultisigRecipientScanner, recipientScanner)
        } else {
            ScrollView { missingIdentity.padding(Spacing.wowPay.page) }.wowPayBackground()
        }
    }

    private var missingIdentity: some View {
        VStack(spacing: Spacing.wowPay.card) {
            if model.isSelecting {
                LoadingView()
                    .accessibilityIdentifier("collectiveLoading")
            } else {
                Text(model.errorMessage ?? Localized.multisigNoLocalIdentity)
                    .font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                    .accessibilityIdentifier(model.errorMessage == nil ? "collectiveNoIdentity" : "collectiveError")
                if model.errorMessage != nil {
                    Button(Localized.Common.tryAgain) { model.refresh(afterUnlock: waitUntilUnlocked) }.buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("collectiveRetry")
                }
                Button(Localized.multisigCreateDedicatedIdentity, action: model.presentDedicatedCreation).buttonStyle(.wowPay())
                NavigationLink(Localized.multisigRestoreIdentity, value: Destination.restore).buttonStyle(.wowPaySecondary())
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var initialRoute: BsvMultisigRoute {
        switch model.entry {
        case .create: .create
        case .importChoice, .join: .join
        case .list: .list
        }
    }

    private func identityContext(for route: BsvMultisigRoute) -> BsvMultisigIdentityContext? {
        model.context(for: route) { id in
            switch route {
            case .createReview, .joinReview, .review, .fundingReview:
                if !path.isEmpty {
                    path.removeLast()
                }
            default: break
            }
            model.selectSigner(id, for: route, afterUnlock: waitUntilUnlocked)
        }
    }

    private var scanner: BsvMultisigInvitationScanner? {
        guard let navigationOwner else { return nil }
        return { completion in model.scanInvitation(navigation: navigationOwner, completion: completion) }
    }

    private var recipientScanner: BsvMultisigRecipientScanner? {
        guard let navigationOwner else { return nil }
        return { completion in model.scanRecipient(navigation: navigationOwner, completion: completion) }
    }

    private var modalBinding: Binding<CollectiveOnboardingModel.IdentityModal?> {
        let presentedId = model.identityModal?.id
        return Binding(get: { model.identityModal }, set: {
            if $0 == nil, let presentedId {
                model.closeIdentityModal(id: presentedId)
            }
        })
    }

    private func waitUntilUnlocked() async {
        await rootModel.lockManager.lockModel.waitUntilUnlocked()
    }
}
