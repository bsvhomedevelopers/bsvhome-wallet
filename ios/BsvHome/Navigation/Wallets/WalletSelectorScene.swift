import BsvColdWallet
import BsvMultisig
import Components
import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeBsvMultisigWallet
import BsvHomeCorePrimitives
import Localization
import ManageWallets
import Onboarding
import Primitives
import PrimitivesComponents
import Store
import Style
import SwiftUI

struct WalletSelectorScene: View {
    private enum Destination: Hashable, Codable {
        case management
    }

    @Environment(\.viewModelFactory) private var factory
    @Environment(\.scenePhase) private var scenePhase
    @Environment(RootSceneViewModel.self) private var rootModel
    let directory: CollectiveWalletDirectory
    let presentation: WalletPresentation
    let navigation: NavigationPathState
    let onDismiss: () -> Void
    let onSelectLocal: () -> Void
    let onSelectCollective: (WalletId, String) -> Void
    let onRefresh: @MainActor () async -> Void

    @State private var model: WalletsSceneViewModel?
    @State private var isCreating = false
    @State private var isImporting = false
    @State private var createModel: CreateWalletModel?
    @State private var importModel: ImportWalletViewModel?
    @State private var createCollectiveModel: CollectiveOnboardingModel?
    @State private var importCollectiveModel: CollectiveOnboardingModel?

    private func coldWalletNavigation(_ content: AnyView, navigation: NavigationPathState) -> AnyView {
        AnyView(content.bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigation)?.model, navigation: navigation, onInitialPageBack: rootModel.coldWalletContext(in: navigation)?.onInitialPageBack))
    }

    var body: some View {
        Group {
            if let model {
                walletList(model)
            } else {
                LoadingView()
            }
        }
        .navigationTitle(Localized.Wallets.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                NavigationLink(value: Destination.management) {
                    Image(systemName: "gearshape")
                }
                .accessibilityLabel(Localized.wowpayWalletManagement)
                .accessibilityIdentifier("manageWallets")
            }
        }
        .navigationDestination(for: Destination.self) { _ in
            if let model {
                WalletsScene(model: model, onSelect: selectLocal)
            }
        }
        .navigationDestination(for: Scenes.WalletDetail.self) {
            WalletDetailScene(model: factory.walletDetailScene(navigationPath: navigation.binding, wallet: $0.wallet, onOpenPairing: { wallet in
                navigation.append(Scenes.BsvColdWallet(walletId: wallet.id.id, entry: .pairing))
            }, onOpenSigning: { wallet in
                navigation.append(Scenes.BsvColdWallet(walletId: wallet.id.id, entry: .signing))
            }))
        }
        .bsvColdWalletNavigation(model: rootModel.coldWalletContext(in: navigation)?.model, navigation: navigation, onScanRecipient: rootModel.coldWalletContext(in: navigation)?.onScanRecipient, onInitialPageBack: rootModel.coldWalletContext(in: navigation)?.onInitialPageBack)
        .navigationDestination(for: Scenes.BsvColdWallet.self) { ColdWalletDestination(scene: $0, navigation: navigation) }
        .navigationDestination(for: Scenes.WalletSelectImage.self) {
            WalletImageScene(model: factory.walletImageScene(wallet: $0.wallet))
        }
        .sheet(isPresented: createBinding) {
            if let createModel {
                CreateWalletNavigationStack(model: createModel, coldWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .create, owner: ObjectIdentifier(createModel), navigation: path, onComplete: selectLocal))
                }, multisigWalletContent: { path in
                    AnyView(Group {
                        if let createCollectiveModel {
                            CollectiveOnboardingScene(model: createCollectiveModel, path: path)
                        }
                    })
                }, onMultisigDismiss: { createCollectiveModel?.cancel() }, onColdWalletDismiss: { rootModel.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(createModel)) }, navigationContent: coldWalletNavigation)
            }
        }
        .sheet(isPresented: importBinding) {
            if let importModel {
                ImportWalletNavigationStack(model: importModel, coldWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .recover, owner: ObjectIdentifier(importModel), navigation: path, onComplete: selectLocal))
                }, connectColdWalletDestination: { path in
                    AnyView(ColdWalletOnboardingDestination(entry: .connect, owner: ObjectIdentifier(importModel), navigation: path, onComplete: selectLocal))
                }, multisigWalletContent: { path in
                    AnyView(Group {
                        if let importCollectiveModel {
                            CollectiveOnboardingScene(model: importCollectiveModel, path: path)
                        }
                    })
                }, onMultisigDismiss: { importCollectiveModel?.cancel() }, onColdWalletDismiss: { rootModel.cancelColdWalletContextForNavigationRemoval(owner: ObjectIdentifier(importModel)) }, navigationContent: coldWalletNavigation)
            }
        }
        .task {
            if model == nil {
                model = factory.walletsScene(navigationPath: navigation.binding, isPresentingCreateWalletSheet: createBinding, isPresentingImportWalletSheet: importBinding)
            }
            await onRefresh()
        }
        .task(id: isCreating || isImporting) {
            guard !isCreating, !isImporting, model != nil, !Task.isCancelled else { return }
            await onRefresh()
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                createBinding.wrappedValue = false
                importBinding.wrappedValue = false
            }
        }
        .onChange(of: rootModel.activeColdWalletContext?.id) { previous, current in
            if previous != nil, current == nil {
                model?.refreshColdRoles()
            }
        }
    }

    private func walletList(_ model: WalletsSceneViewModel) -> some View {
        @Bindable var model = model
        return ScrollView {
            VStack(alignment: .leading, spacing: .zero) {
                VStack(alignment: .leading, spacing: Spacing.wowPay.section) {
                    VStack(alignment: .leading, spacing: Spacing.tiny) {
                        Text(Localized.coldWalletChoose)
                            .font(.wowPay.walletPickerTitle)
                            .foregroundStyle(Colors.WowPay.heading)
                        Text(Localized.wowpayWalletPickerSubtitle)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                    }
                    if !model.personalPinnedWallets.isEmpty || !model.personalWallets.isEmpty {
                        VStack(alignment: .leading, spacing: Spacing.wowPay.page) {
                            sectionHeader(Localized.wowpayWalletPickerPersonal)
                            VStack(spacing: Spacing.medium) {
                                ForEach(model.personalPinnedWallets) { wallet in
                                    localRow(wallet, model: model)
                                }
                                ForEach(model.personalWallets) { wallet in
                                    localRow(wallet, model: model)
                                }
                            }
                        }
                    }
                    if !model.coldSignerWallets.isEmpty {
                        VStack(alignment: .leading, spacing: Spacing.wowPay.page) {
                            sectionHeader(Localized.wowpayWalletPickerCold)
                            VStack(spacing: Spacing.medium) {
                                ForEach(model.coldSignerWallets) { wallet in localRow(wallet, model: model) }
                            }
                        }
                    }
                    if !directory.wallets.isEmpty {
                        VStack(alignment: .leading, spacing: Spacing.wowPay.page) {
                            sectionHeader(Localized.wowpayWalletPickerCollective, count: Int(directory.overview.pendingCount))
                            VStack(spacing: Spacing.medium) {
                                ForEach(directory.wallets, id: \.id) { wallet in
                                    collectiveRow(wallet)
                                }
                            }
                        }
                    }
                    if let error = model.coldRoleError {
                        Text(error).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                        Button(Localized.Common.tryAgain) { model.refreshColdRoles() }
                            .buttonStyle(.wowPaySecondary())
                    } else if !model.coldRolesReady {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                    if directory.isLoading {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                    if let error = directory.errorMessage {
                        Text(error).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                        Button(Localized.Common.tryAgain) { Task { await onRefresh() } }
                            .buttonStyle(.wowPaySecondary())
                    }
                }
                HStack(spacing: Spacing.wowPay.grid) {
                    Button(Localized.wowpayCreateAction) { createBinding.wrappedValue = true }
                        .buttonStyle(.wowPay())
                        .accessibilityIdentifier("createLocalWallet")
                    Button(Localized.Wallet.Import.title) { importBinding.wrappedValue = true }
                        .buttonStyle(.wowPaySecondary())
                        .accessibilityIdentifier("importLocalWallet")
                }
                .padding(.top, Spacing.wowPay.page)
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .refreshable { await onRefresh() }
        .bindQuery(model.pinnedWalletsQuery, model.walletsQuery)
        .task(id: model.coldRoleInput) { model.refreshColdRoles() }
        .alertSheet($model.isPresentingAlertMessage)
    }

    private func sectionHeader(_ title: String, count: Int = 0) -> some View {
        HStack(spacing: Spacing.small) {
            Text(title)
                .font(.wowPay.walletPickerSection)
                .foregroundStyle(Colors.WowPay.heading)
                .accessibilityAddTraits(.isHeader)
            Spacer(minLength: Spacing.small)
            WowPayCountBadge(count: count)
        }
    }

    private func localRow(_ wallet: Wallet, model: WalletsSceneViewModel) -> some View {
        let isColdSigner = model.coldRole(for: wallet) == .signer
        let subtitle = isColdSigner ? Localized.wowpayWalletPickerColdSigning : wallet.type == .view ? Localized.wowpayWalletPickerObserver : Localized.wowpayWalletPickerPersonalSigning
        let address = isColdSigner ? nil : wallet.accounts.first { $0.chain == .bitcoinSv }.map {
            BsvHomeAddressService.shared.format(address: $0.address, chain: .bitcoinSv, style: .extra(extra: 1))
        }
        return Button {
            model.onSelect(wallet: wallet, onSelected: selectLocal)
        } label: {
            WowPaySelectionRow(title: wallet.name.isEmpty ? Localized.Wallets.title : wallet.name, subtitle: subtitle, image: Image(isColdSigner ? "wowpay-shield-check" : "wowpay-wallet-outline"), isSelected: presentation == .local && wallet.id == model.currentWalletId, style: .stacked, detail: address)
        }
        .buttonStyle(.plain)
        .disabled(!model.coldRolesReady)
        .accessibilityIdentifier("selectLocalWallet_" + wallet.id.id)
    }

    private func collectiveRow(_ wallet: BsvHomeBsvMultisigWallet) -> some View {
        let isSelected = if case let .collective(_, id) = presentation {
            id == wallet.id
        } else {
            false
        }
        return Button {
            guard let id = wallet.localSignerWalletId, let signer = try? WalletId.from(id: id) else { return }
            onDismiss()
            onSelectCollective(signer, wallet.id)
        } label: {
            WowPaySelectionRow(title: wallet.name, subtitle: Localized.wowpayWalletPickerMultisigSummary(Int(wallet.threshold), Int(wallet.memberCount), wallet.statusTitle, Int(wallet.pendingCount)), image: Image("wowpay-shield-check"), isSelected: isSelected, style: .stacked, detail: Localized.wowpayWalletPickerMultisigDetail, trailingImage: Image("wowpay-chevron-right"))
        }
        .buttonStyle(.plain)
        .disabled(wallet.localSignerWalletId == nil)
        .accessibilityIdentifier("selectCollectiveWallet_" + wallet.id)
    }

    private func selectLocal() {
        createBinding.wrappedValue = false
        importBinding.wrappedValue = false
        onDismiss()
        onSelectLocal()
    }

    private var createBinding: Binding<Bool> {
        Binding(get: { isCreating }, set: {
            if $0, createModel == nil {
                createModel = factory.createWalletScene(onComplete: { selectLocal() })
                createCollectiveModel = rootModel.makeCollectiveOnboarding(entry: .create, directory: directory)
            } else if !$0 {
                if let createModel {
                    rootModel.cancelColdWalletContext(owner: ObjectIdentifier(createModel))
                }
                createModel?.cancel()
                createCollectiveModel?.cancel()
                createModel = nil
                createCollectiveModel = nil
            }
            isCreating = $0
        })
    }

    private var importBinding: Binding<Bool> {
        Binding(get: { isImporting }, set: {
            if $0, importModel == nil {
                importModel = factory.importWalletScene(onComplete: { selectLocal() })
                importCollectiveModel = rootModel.makeCollectiveOnboarding(entry: .importChoice, directory: directory)
            } else if !$0 {
                if let importModel {
                    rootModel.cancelColdWalletContext(owner: ObjectIdentifier(importModel))
                }
                importModel?.cancel()
                importCollectiveModel?.cancel()
                importModel = nil
                importCollectiveModel = nil
            }
            isImporting = $0
        })
    }
}
