import Components
import Localization
import PhaseOne
import PrimitivesComponents
import QRScanner
import Store
import Style
import SwiftUI

struct PhaseOneDestination: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.navigationState) private var navigationState
    @Environment(\.viewModelFactory) private var viewModelFactory
    let factory: PhaseOneServicesFactory
    let authenticate: @MainActor @Sendable () async throws -> Void
    var onCreateLocalWallet: (() -> Void)?
    var onImportLocalWallet: (() -> Void)?
    var onUseLocalWallet: (() -> Void)?

    @State private var models: PhaseOneModels?
    @State private var failed = false
    @State private var captured = UIScreen.main.isCaptured
    @State private var selectedWallet: String?
    @State private var selectedTab = 0
    @State private var isLoading = false

    private var isRoot: Bool { onCreateLocalWallet != nil }

    var body: some View {
        ZStack {
            if let models {
                content(models)
                    .safeAreaInset(edge: .top) {
                        if failed {
                            HStack {
                                Text(Localized.phaseoneStorageUnavailable)
                                Button(Localized.Common.tryAgain) { Task { await load() } }
                                    .disabled(isLoading)
                            }
                            .font(.wowPay.caption)
                            .padding(Spacing.wowPay.page)
                            .background(Colors.WowPay.surface)
                        }
                    }
            } else if failed {
                VStack(spacing: Spacing.wowPay.card) {
                    Text(Localized.phaseoneStorageUnavailable)
                    Button(Localized.Common.tryAgain) { failed = false; Task { await load() } }
                    if let onUseLocalWallet {
                        Button(Localized.Transfer.Recipient.myWallets, action: onUseLocalWallet)
                    }
                }.padding(Spacing.wowPay.page)
            } else {
                ProgressView()
            }
        }
        .tint(Colors.WowPay.brand)
        .task { await load() }
        .onChange(of: models?.account.session?.userId) { _, _ in
            models?.mpc.clearSession()
            models?.hd.clearMemory()
            models?.alternative.clearMemory()
            models?.recipientInputs.cancel()
            selectedWallet = nil
            selectedTab = 0
        }
        .onChange(of: selectedWallet) { _, id in
            models?.recipientInputs.cancel()
            models?.mpc.selectWallet(id)
        }
        .onChange(of: models?.mpc.selectedWalletId) { _, id in
            selectedWallet = id
            if let id {
                navigationState.walletPresentation = .mpc(walletId: id)
            }
        }
        .onChange(of: models?.mpc.wallets.first?.walletId) { _, id in
            if isRoot, !isLoading, selectedWallet == nil {
                selectedWallet = id
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                suspend()
            }
            if phase == .active {
                Task { await load() }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in
            captured = UIScreen.main.isCaptured
            if captured {
                suspend()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.userDidTakeScreenshotNotification)) { _ in suspend() }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataWillBecomeUnavailableNotification)) { _ in suspend() }
        .overlay {
            if scenePhase != .active || captured {
                Colors.WowPay.surface.ignoresSafeArea()
                    .overlay { Text(Localized.SecretPhrase.ContentHidden.description).padding() }
            }
        }
        .onDisappear { suspend() }
    }

    private func content(_ models: PhaseOneModels) -> some View {
        @Bindable var inputs = models.recipientInputs
        return Group {
            if isRoot, models.account.session != nil {
                TabView(selection: $selectedTab) {
                    NavigationStack { mpcScene(models) }
                        .tabItem { Label(Localized.wowpayHome, systemImage: "house") }.tag(0)
                    NavigationStack {
                        VStack(spacing: .zero) {
                            if !models.mpc.wallets.isEmpty {
                                Picker(Localized.Wallets.title, selection: $selectedWallet) {
                                    ForEach(models.mpc.wallets, id: \.walletId) { wallet in Text(wallet.name).tag(Optional(wallet.walletId)) }
                                }.pickerStyle(.menu)
                            }
                            mpcScene(models, walletId: selectedWallet)
                        }
                    }
                    .id(selectedWallet)
                    .tabItem { Label(Localized.Assets.title, systemImage: "wallet.bifold") }.tag(1)
                    accountNavigation(models)
                        .tabItem { Label(Localized.wowpayProfile, systemImage: "person") }.tag(2)
                }
            } else {
                accountNavigation(models)
            }
        }
        .bindQuery(inputs.contactsQuery)
        .onChange(of: inputs.contactsQuery.value) { Task { await inputs.refreshContacts() } }
        .sheet(item: $inputs.presentation) { request in
            switch request.kind {
            case .scanner:
                ScanQRCodeNavigationStack(scanType: .address) { inputs.scanned($0, request: request) }
            case .contacts:
                switch inputs.contacts {
                case .noData, .loading: ProgressView()
                case let .data(rows):
                    ContactRecipientPicker(recipients: rows, onSelect: { inputs.select($0, request: request) }, onClose: inputs.cancel)
                case .error:
                    VStack(spacing: Spacing.wowPay.card) {
                        Text(Localized.phaseoneServiceError)
                        Button(Localized.Common.tryAgain) { Task { await inputs.refreshContacts() } }
                        Button(Localized.Common.done, action: inputs.cancel)
                    }.padding(Spacing.wowPay.page)
                }
            }
        }
        .alert(Localized.Errors.errorOccurred, isPresented: Binding(get: { inputs.error != nil }, set: {
            if !$0 {
                inputs.error = nil
            }
        })) {
            Button(Localized.Common.done) { inputs.error = nil }
        } message: { Text(inputs.error ?? "") }
    }

    private func mpcScene(_ models: PhaseOneModels, walletId: String? = nil) -> some View {
        @Bindable var preferences = viewModelFactory.observablePreferences
        return MpcWalletScene(model: models.mpc, initialWalletId: walletId, isBalanceHidden: $preferences.isHideBalanceEnabled, onScan: models.recipientInputs.presentScanner, onContacts: models.recipientInputs.presentContacts)
    }

    private func accountNavigation(_ models: PhaseOneModels) -> some View {
        NavigationStack {
            PlatformAccountScene(model: models.account) { mpcScene(models) } advanced: {
                if models.account.session != nil {
                    NavigationLink(Localized.phaseoneHd) { PlatformHdScene(model: models.hd) }
                }
                NavigationLink(Localized.phaseoneAlternative) { PlatformAlternativeScene(model: models.alternative) }
                    .accessibilityIdentifier("phaseOneAlternativeRecovery")
                if let onCreateLocalWallet {
                    Button(Localized.Wallet.createNewWallet, action: onCreateLocalWallet)
                        .accessibilityIdentifier("createLocalWallet")
                }
                if let onImportLocalWallet {
                    Button(Localized.Wallet.importExistingWallet, action: onImportLocalWallet)
                        .accessibilityIdentifier("importLocalWallet")
                }
            }
            .toolbar {
                if !isRoot {
                    ToolbarItem(placement: .topBarLeading) { Button(Localized.Common.done) { dismiss() } }
                }
                if let onUseLocalWallet {
                    ToolbarItem(placement: .topBarTrailing) { Button(Localized.Transfer.Recipient.myWallets, action: onUseLocalWallet) }
                }
            }
        }
    }

    private func load() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        let preferred: String? = switch navigationState.walletPresentation {
        case .local, .collective, .collectiveOverview: selectedWallet
        case let .mpc(walletId): walletId
        }
        do {
            let services = try await factory.sharedServices()
            failed = false
            if models == nil {
                models = PhaseOneModels(services: services, authenticate: authenticate, onWalletChange: factory.suspendAndWait)
            }
            guard let models else { return }
            await models.account.load()
            if isRoot, models.account.session != nil {
                await models.mpc.load()
                let candidates = [models.mpc.selectedWalletId, preferred].compactMap(\.self)
                let chosen = candidates.first(where: { candidate in models.mpc.wallets.contains(where: { $0.walletId == candidate }) }) ?? models.mpc.wallets.first?.walletId
                models.mpc.selectWallet(chosen)
                selectedWallet = chosen
            }
        } catch { failed = true }
    }

    private func suspend() {
        models?.mpc.clearSensitive()
        models?.account.clearInput()
        models?.hd.clearMemory()
        models?.alternative.clearMemory()
        models?.recipientInputs.cancel()
        factory.suspend()
    }
}
