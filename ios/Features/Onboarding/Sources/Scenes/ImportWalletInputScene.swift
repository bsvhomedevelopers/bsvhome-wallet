import Components
import BsvHomeCorePrimitives
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import QRScanner
import Style
import SwiftUI

public struct ImportWalletInputScene: View {
    private let kind: ImportWalletInputKind
    private let importModel: ImportWalletViewModel
    private let onNotifications: (() -> Void)?
    private let onNewWallet: @MainActor @Sendable (Wallet) -> Void
    private let onExistingWallet: @MainActor @Sendable () -> Void

    @State private var model: ImportWalletSceneViewModel?
    @State private var availableChains: [Chain] = []
    @State private var selectedChain: Chain?
    @State private var isSelectingNetwork = false
    @State private var selectedWordCount = 12
    @State private var acceptedTermsThisSession = false
    @FocusState private var isInputFocused: Bool

    public init(
        kind: ImportWalletInputKind,
        importModel: ImportWalletViewModel,
        acceptedTermsThisSession: Bool = false,
        onNotifications: (() -> Void)? = nil,
        onNewWallet: @escaping @MainActor @Sendable (Wallet) -> Void,
        onExistingWallet: @escaping @MainActor @Sendable () -> Void,
    ) {
        self.kind = kind
        self.importModel = importModel
        _acceptedTermsThisSession = State(initialValue: acceptedTermsThisSession)
        self.onNotifications = onNotifications
        self.onNewWallet = onNewWallet
        self.onExistingWallet = onExistingWallet
    }

    public var body: some View {
        if importModel.isAcceptTermsCompleted || acceptedTermsThisSession {
            inputPage
        } else {
            AcceptTermsScene(model: AcceptTermsViewModel(onNext: { acceptedTermsThisSession = true }))
                .toolbar(.visible, for: .navigationBar)
        }
    }

    private var inputPage: some View {
        ScrollView {
            VStack(spacing: .zero) {
                VStack(spacing: Spacing.space24) {
                    Text(title)
                        .font(.wowPay.onboardingTitle)
                        .foregroundStyle(Colors.WowPay.heading)
                    Text(subtitle)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.top, Spacing.wowPay.welcomeVertical)

                VStack(spacing: Spacing.wowPay.card) {
                    if kind == .phrase {
                        wordCountSelector
                    } else {
                        networkSelectorRow
                    }

                    inputField

                    if kind == .phrase, let model, model.showsWordSuggestions, isInputFocused, !model.wordsSuggestion.isEmpty {
                        WordSuggestionView(words: model.wordsSuggestion, selectWord: model.onSelectWord)
                    }

                    if let model, model.showsHdSettings {
                        @Bindable var model = model
                        BsvHdImportSettings(configuration: $model.hdConfiguration, passphrase: $model.passphrase, isSecret: model.shouldProtectInput)
                            .disabled(model.isBusy)
                        if model.shouldProtectInput {
                            Text(Localized.bsvHdBsvOnlyHint).font(.wowPay.caption).foregroundStyle(Colors.WowPay.secondaryText)
                        }
                    }

                    if showsWordCountMismatch {
                        Text(Localized.wowpayImportWordCountMismatch)
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    if let note {
                        Text(.init(note))
                            .font(.wowPay.caption)
                            .foregroundStyle(Colors.WowPay.secondaryText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    VStack(spacing: Spacing.wowPay.grid) {
                        Button(Localized.Common.paste, action: onPaste)
                            .buttonStyle(.wowPaySecondary())
                            .disabled(model == nil || isImporting || model?.hasPendingIdentityImport == true)
                            .accessibilityIdentifier("wowPayImportInputPaste")

                        if kind != .phrase, let model {
                            Button(Localized.Wallet.scan, action: model.onSelectScanQR)
                                .buttonStyle(.wowPaySecondary())
                                .disabled(isImporting)
                        }

                        StateButton(
                            text: model?.hasPendingIdentityImport == true ? Localized.Common.tryAgain : Localized.Wallet.Import.action,
                            type: .wowPayPrimary(isImporting || canSubmit ? model?.buttonState ?? .disabled : .disabled),
                            action: onImport,
                        )
                        .accessibilityIdentifier("wowPayImportInputSubmit")
                    }
                    .frame(maxWidth: .scene.button.maxWidth)
                }
                .padding(.top, Spacing.wowPay.welcomeVertical)
            }
            .frame(maxWidth: .infinity)
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle(Localized.Wallet.Import.title)
        .toolbarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar {
            if let onNotifications {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: onNotifications) {
                        Image("wowpay-bell")
                            .resizable()
                            .scaledToFit()
                            .frame(width: Sizing.wowPay.icon, height: Sizing.wowPay.icon)
                    }
                    .accessibilityLabel(Localized.Settings.Notifications.title)
                    .accessibilityIdentifier("wowPayImportInputNotifications")
                }
            }
        }
        .navigationDestination(isPresented: $isSelectingNetwork) {
            NetworkSelectorScene(model: networkSelectorModel, onFinishSelection: onSelectNetwork)
        }
        .alertSheet(alertBinding)
        .sheet(isPresented: existingWalletBinding) {
            if let model, let walletName = model.isPresentingExistingWalletName {
                InfoSheetScene(
                    model: InfoSheetModel(
                        title: walletName,
                        description: Localized.Wallet.Import.alreadyImportedMessage,
                        image: .image(Images.Logo.logo),
                        button: .action(
                            title: Localized.Common.continue,
                            action: model.onSelectExistingWalletContinue,
                        ),
                    ),
                )
            }
        }
        .onAppear(perform: prepare)
        .onChange(of: model?.input) { oldValue, newValue in
            guard let model, let newValue else { return }
            model.onChangeInput(oldValue ?? "", newValue: newValue)
        }
        .detectScreenshots(docsUrl: AppUrl.docs(.howToSecureSecretPhrase))
        .modifier(ImportWalletProtection(model: model))
        .navigationDestination(isPresented: hdReviewBinding) {
            if let model {
                BsvHdImportReviewScene(model: model)
            }
        }
        .sheet(isPresented: scannerBinding) {
            if let model {
                ScanQRCodeNavigationStack(scanType: model.scanType, mode: kind == .address && selectedChain == .bitcoinSv ? .continuous : .single, hint: model.scanHint, action: { value in Task { await model.onHandleScan(value) } })
            }
        }
        .onDisappear {
            if model?.isPresentingHdReview != true, !isSelectingNetwork, model?.isPresentingScanner != true {
                model?.clearSensitiveState()
            }
        }
    }

    private var title: String {
        switch kind {
        case .phrase: Localized.wowpayImportPhraseTitle
        case .privateKey: Localized.wowpayImportPrivateKeyTitle
        case .address: Localized.wowpayImportWatchTitle
        }
    }

    private var subtitle: String {
        switch kind {
        case .phrase: Localized.wowpayImportPhraseSubtitle
        case .privateKey: Localized.wowpayImportPrivateKeySubtitle
        case .address: Localized.wowpayImportWatchSubtitle
        }
    }

    private var placeholder: String {
        switch kind {
        case .phrase: Localized.wowpayImportPhrasePlaceholder
        case .privateKey: Localized.wowpayImportPrivateKeyPlaceholder
        case .address: Localized.wowpayImportWatchPlaceholder
        }
    }

    private var note: String? {
        switch kind {
        case .phrase: Localized.wowpayImportPhraseSecurityNote
        case .privateKey: Localized.wowpayImportPrivateKeySecurityNote
        case .address: Localized.Wallet.importAddressWarning
        }
    }

    private var inputBinding: Binding<String> {
        Binding(
            get: { model?.input ?? "" },
            set: { model?.input = $0 },
        )
    }

    private var alertBinding: Binding<AlertMessage?> {
        Binding(
            get: { model?.isPresentingAlertMessage },
            set: { model?.isPresentingAlertMessage = $0 },
        )
    }

    private var existingWalletBinding: Binding<Bool> {
        Binding(
            get: { model?.isPresentingExistingWalletName != nil },
            set: {
                if !$0 {
                    model?.isPresentingExistingWalletName = nil
                }
            },
        )
    }

    private var networkSelectorModel: NetworkSelectorViewModel {
        NetworkSelectorViewModel(
            state: .data(.plain(availableChains)),
            selectedItems: selectedChain.map { [$0] } ?? [],
            selectionType: .checkmark,
            title: Localized.Transfer.network,
        )
    }

    private var inputWordCount: Int {
        model?.input.split(whereSeparator: \.isWhitespace).count ?? 0
    }

    private var showsWordCountMismatch: Bool {
        kind == .phrase && model?.hasRecoveryBackupInput != true && inputWordCount > 0 && inputWordCount != selectedWordCount
    }

    private var canSubmit: Bool {
        guard let model, importModel.isAcceptTermsCompleted || acceptedTermsThisSession else { return false }
        return model.canSubmit && (model.hasPendingIdentityImport || model.requiresHdPreview || kind.canImport(input: model.input, selectedWordCount: selectedWordCount, selectedChain: selectedChain))
    }

    private var isImporting: Bool {
        guard let model else { return false }
        if case .loading = model.buttonState {
            return true
        }
        return false
    }

    private var wordCountSelector: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: Spacing.space6), count: 3), spacing: Spacing.space6) {
            ForEach([12, 15, 18, 21, 24], id: \.self) { count in
                Button {
                    selectedWordCount = count
                } label: {
                    Text(Localized.wowpayImportWordCount(String(count)))
                        .font(.wowPay.label)
                        .foregroundStyle(selectedWordCount == count ? Colors.WowPay.onBrand : Colors.WowPay.secondaryText)
                        .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.buttonHeight)
                        .background {
                            if selectedWordCount == count {
                                RoundedRectangle(cornerRadius: Sizing.wowPay.inputCorner)
                                    .fill(WowPayGradients.primaryButton)
                            }
                        }
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(selectedWordCount == count ? .isSelected : [])
                .accessibilityIdentifier("wowPayImportWordCount\(count)")
            }
        }
        .disabled(model?.hasPendingIdentityImport == true)
        .padding(Spacing.space6)
        .background(Colors.WowPay.pale, in: RoundedRectangle(cornerRadius: Sizing.wowPay.controlCorner))
    }

    private var networkSelectorRow: some View {
        Button {
            isInputFocused = false
            isSelectingNetwork = true
        } label: {
            HStack(spacing: Spacing.wowPay.grid) {
                Text(Localized.Transfer.network)
                    .font(.wowPay.label)
                    .foregroundStyle(Colors.WowPay.heading)
                Spacer()
                if let selectedChain {
                    Text(selectedChain.networkName)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.heading)
                } else {
                    Text(Localized.wowpayImportSelectNetwork)
                        .font(.wowPay.subtitle)
                        .foregroundStyle(Colors.WowPay.secondaryText)
                }
                Image("wowpay-chevron-right")
                    .resizable()
                    .scaledToFit()
                    .frame(width: Sizing.wowPay.chevron, height: Sizing.wowPay.chevron)
            }
            .frame(minHeight: Sizing.wowPay.buttonHeight)
            .wowPayInput()
        }
        .buttonStyle(.plain)
        .disabled(isImporting || availableChains.isEmpty)
        .accessibilityIdentifier("wowPayImportSelectNetwork")
    }

    private var inputField: some View {
        HStack(alignment: .top) {
            WalletImportInputField(
                placeholder: placeholder,
                isSecret: kind != .address,
                isDisabled: model == nil || isImporting || model?.hasPendingIdentityImport == true,
                identifier: "wowPayImportInputField",
                focus: $isInputFocused,
                input: inputBinding,
                isSecretVisible: Binding(get: { model?.isSecretVisible ?? false }, set: { model?.isSecretVisible = $0 }),
            )
            if let model, kind == .address, !model.requiresHdPreview, let nameRecordViewModel = model.nameRecordViewModel {
                NameRecordView(model: nameRecordViewModel)
            }
        }
        .wowPayInput()
    }

    private var hdReviewBinding: Binding<Bool> {
        Binding(get: { model?.isPresentingHdReview ?? false }, set: { model?.isPresentingHdReview = $0 })
    }

    private var scannerBinding: Binding<Bool> {
        Binding(get: { model?.isPresentingScanner ?? false }, set: { model?.isPresentingScanner = $0 })
    }

    private func prepare() {
        if kind == .phrase, model == nil {
            installModel(type: .multicoin)
        }
        if kind != .phrase, availableChains.isEmpty {
            availableChains = importModel.supportedChains(for: kind.importKind)
        }
        if let model {
            importModel.registerInputModel(model)
        }
    }

    private func installModel(type: ImportWalletType) {
        let onNewWallet = onNewWallet
        let onExistingWallet = onExistingWallet
        let newModel = importModel.importWalletModel(type: type) { result in
            switch result {
            case let .new(wallet): onNewWallet(wallet)
            case .existing: onExistingWallet()
            }
        }
        newModel.importType = kind.importKind
        model?.clearSensitiveState()
        model = newModel
        importModel.registerInputModel(newModel)
    }

    private func onSelectNetwork(_ chains: [Chain]) {
        guard let chain = chains.first, availableChains.contains(chain), chain != selectedChain else { return }
        selectedChain = chain
        installModel(type: .chain(chain))
    }

    private func onPaste() {
        model?.onPaste()
    }

    private func onImport() {
        guard canSubmit, let model else { return }
        isInputFocused = false
        model.startImport()
    }
}
