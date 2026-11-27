// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.Chain
import struct BsvHomeCore.BsvHomeAppStartFailure
import protocol BsvHomeCore.BsvHomeAppStartServiceProtocol
import enum BsvHomeCore.BsvHomeBsvHdImportMode
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import protocol BsvHomeCore.BsvHomePreferencesStore
import enum BsvHomeCore.BsvHomeServiceError
import struct BsvHomeCore.BsvHomeWalletDefaultName
import enum BsvHomeCore.BsvHomeWalletImportKind
import enum BsvHomeCore.BsvHomeWalletImportResult
import class BsvHomeCore.BsvHomeWalletService
import class BsvHomeCore.BsvHomeWalletSessionService
import struct BsvHomeCore.Wallet
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import Onboarding
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
import Store
import StoreTestKit
import Testing

@MainActor
struct ImportWalletSceneViewModelTests {
    @Test(arguments: [false, true])
    func parentCancellationPreventsFirstPollAndLateIdentityRecovery(cancelBeforeFirstPoll: Bool) async throws {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        service.completionError = nil
        let pending = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        if !cancelBeforeFirstPoll {
            service.onImport = { await withCheckedContinuation { pending.continuation.yield($0) } }
        }
        let identityCompletion = ImportCompletionSpy()
        let parent = ImportWalletViewModel(service: service, keystore: LocalKeystore.mock().bsvHomeKeystore, authenticate: {}, appStartService: WalletAssetsSetupMock(), preferences: .mock(), nameService: BsvHomeNameServiceMock(), avatarService: BsvHomeAvatarServiceMock(), onComplete: nil, purpose: .dedicatedMultisigIdentity { _ in identityCompletion.count += 1 })
        let model = parent.importWalletModel(type: .multicoin, onComplete: { _ in Issue.record("A dedicated recovery must not complete the personal flow") })
        parent.registerInputModel(model)
        model.input = LocalKeystore.words.joined(separator: " ")
        model.startImport()
        let task = try #require(model.importTask)
        var requests = pending.stream.makeAsyncIterator()
        let completion: CheckedContinuation<Void, Never>? = if cancelBeforeFirstPoll {
            nil
        } else {
            try #require(await requests.next())
        }
        _ = parent.importWalletModel(type: .multicoin, onComplete: { _ in })
        parent.cancel()
        completion?.resume()
        await task.value

        #expect(task.isCancelled)
        #expect(service.importCount == (cancelBeforeFirstPoll ? 0 : 1))
        #expect(service.completionResults.isEmpty)
        #expect(service.selectedWalletIds.isEmpty)
        #expect(identityCompletion.count == 0)
        #expect(model.input.isEmpty && model.passphrase.isEmpty)
        #expect(model.importTask == nil)
    }

    @Test(arguments: [false, true])
    func dedicatedRecoveryRetriesTheOriginalImportResultWithoutKeepingThePhrase(existing: Bool) async {
        let service = MultisigIdentityWalletServiceMock(noHandle: .init())
        if existing, case let .new(wallet) = service.result {
            service.result = .existing(wallet: wallet)
        }
        let setup = WalletAssetsSetupMock()
        let personalCompletion = ImportCompletionSpy()
        let identityCompletion = ImportCompletionSpy()
        let model = ImportWalletSceneViewModel.mock(
            service: service,
            type: .multicoin,
            appStartService: setup,
            onComplete: { _ in personalCompletion.count += 1 },
            purpose: .dedicatedMultisigIdentity { _ in identityCompletion.count += 1 },
        )
        model.input = LocalKeystore.words.joined(separator: " ")
        await model.onSelectActionButton()

        #expect(service.importCount == 1)
        #expect(service.completionResults == [service.result])
        #expect(model.hasPendingIdentityImport)
        #expect(model.input.isEmpty && model.passphrase.isEmpty)
        #expect(model.isPresentingAlertMessage != nil)
        #expect(model.isPresentingExistingWalletName == nil)
        #expect(model.canSubmit)
        #expect(identityCompletion.count == 0)
        model.clearSensitiveState()

        service.completionError = nil
        await model.onSelectActionButton()

        #expect(service.importCount == 1)
        #expect(service.completionResults == [service.result, service.result])
        #expect(service.completionSources == [.import, .import])
        #expect(service.selectedWalletIds.isEmpty)
        #expect(await setup.wallets.isEmpty)
        #expect(identityCompletion.count == 1)
        #expect(personalCompletion.count == 0)
        #expect(!model.hasPendingIdentityImport)
        #expect(model.input.isEmpty && model.passphrase.isEmpty)
    }

    @Test
    func importSelectorsFollowCoreScopeAndCapabilities() {
        let keystore = LocalKeystore.mock()
        let model = ImportWalletViewModel(
            service: BsvHomeWalletService.mock(keystore: keystore, environment: .wowPay),
            keystore: keystore.bsvHomeKeystore,
            authenticate: {},
            appStartService: WalletAssetsSetupMock(),
            preferences: .mock(),
            nameService: BsvHomeNameServiceMock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: {},
        )

        #expect(Set(model.importWalletTypeModel().items(for: "")) == [.bitcoinSv])
        #expect(model.importWalletTypeModel().items(for: "Ethereum").isEmpty)
        #expect(model.importWalletTypeModel().items(for: "Tron").isEmpty)
        #expect(Set(model.supportedChains(for: .phrase)) == [.bitcoinSv])
        #expect(Set(model.supportedChains(for: .address)) == [.bitcoinSv])
        #expect(Set(model.supportedChains(for: .privateKey)) == [.bitcoinSv])
    }

    @Test
    func existingImportSetsCurrentWallet() async throws {
        let db = DB.mockWithChains([.ethereum, .bitcoinSv, .tron])
        let sessionStore = BsvHomeCoreWalletSessionStore.mock()
        let session = BsvHomeWalletSessionService(store: sessionStore, wallets: BsvHomeCoreWalletStore(store: WalletStore.mock(db: db)))
        let service = BsvHomeWalletService.mock(db: db, sessionStore: sessionStore)

        let walletA = try await service.importWallet(
            name: "Wallet A",
            type: .singlePhrase(words: LocalKeystore.words, chain: Primitives.Chain.ethereum.map()),
            source: .import,
        ).wallet

        let walletB = try await service.importWallet(
            name: "Wallet B",
            type: .singlePhrase(words: service.createWallet(), chain: Primitives.Chain.ethereum.map()),
            source: .import,
        ).wallet
        try await session.setCurrent(wallet: walletB)

        #expect(session.currentWalletId == walletB.id)

        let setup = WalletAssetsSetupMock()
        let model = ImportWalletSceneViewModel.mock(service: service, appStartService: setup)
        model.input = LocalKeystore.words.joined(separator: " ")
        await model.onSelectActionButton()

        #expect(session.currentWalletId == walletA.id)
        #expect(await setup.wallets.map(\.id) == [walletA.id.id])
        await model.onSelectActionButton()
        #expect(await setup.wallets.count == 2)
        #expect(model.isPresentingExistingWalletName == "Wallet A")
    }

    @Test
    func assetSetupFailureKeepsImportOpenAndShowsError() async throws {
        let db = DB.mockWithChains([.ethereum, .bitcoinSv, .tron])
        let service = BsvHomeWalletService.mock(db: db)
        let setup = WalletAssetsSetupMock(error: .Store(msg: "disk full"))
        let model = ImportWalletSceneViewModel.mock(service: service, appStartService: setup)
        model.input = LocalKeystore.words.joined(separator: " ")

        await model.onSelectActionButton()

        #expect(model.isPresentingAlertMessage != nil)
        #expect(model.isPresentingExistingWalletName == nil)
        #expect(try service.currentWalletId() == nil)
        #expect(await setup.wallets.count == 1)
    }

    @Test
    func walletSelectionFailureKeepsImportOpenAndDoesNotComplete() async throws {
        let db = DB.mockWithChains([.ethereum])
        let sessionStore = BsvHomeCoreWalletSessionStore(store: FailingPreferencesStore())
        let service = BsvHomeWalletService.mock(db: db, sessionStore: sessionStore)
        let setup = WalletAssetsSetupMock()
        let completion = ImportCompletionSpy()
        let model = ImportWalletSceneViewModel.mock(
            service: service,
            appStartService: setup,
            onComplete: { _ in completion.count += 1 },
        )
        model.importType = .address
        model.input = "0x0000000000000000000000000000000000000001"

        await model.onSelectActionButton()

        #expect(model.isPresentingAlertMessage != nil)
        #expect(model.isPresentingExistingWalletName == nil)
        #expect(completion.count == 0)
        #expect(try service.currentWalletId() == nil)
        #expect(await setup.wallets.count == 1)
    }

    @Test
    func resolvesNameOnlyForAddressImport() async throws {
        let nameService = BsvHomeNameServiceMock(nameRecord: .mock())
        let model = ImportWalletSceneViewModel.mock(nameService: nameService)

        model.importType = .privateKey
        model.onChangeInput("", newValue: "vitalik.eth")
        #expect(model.nameRecordViewModel?.nameRecordTask == nil)
        #expect(nameService.requestedNames.isEmpty)

        model.importType = .address
        model.onChangeInput("", newValue: "vitalik.eth")
        let resolution = try #require(model.nameRecordViewModel?.nameRecordTask)
        await resolution.value
        #expect(nameService.requestedNames == ["vitalik.eth"])
    }

    @Test
    func hdPreviewRequiresConfirmationAndInvalidatesOnInputOrConfigurationChange() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore)
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .chain(.bitcoinSv))
        model.input = LocalKeystore.words.joined(separator: " ")
        await model.onSelectActionButton()

        #expect(model.preview?.accountPath == "m/44'/236'/0'")
        #expect(model.isPresentingHdReview)
        #expect(!model.isHdConfirmed)
        #expect(try await service.getWallets().isEmpty)
        await model.confirmHdImport()
        #expect(try await service.getWallets().isEmpty)
        model.isHdConfirmed = true
        model.hdConfiguration.pathPreset = .compatible
        #expect(model.preview == nil)
        #expect(!model.isHdConfirmed)
        await model.onSelectActionButton()
        #expect(model.preview?.accountPath == "m/44'/145'/0'")
        model.isHdConfirmed = true
        model.input += " "
        #expect(model.preview == nil)
        #expect(!model.isHdConfirmed)
    }

    @Test
    func clearingPrivateSessionDuringAuthenticationBlocksLateImport() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore)
        let authentication = ImportAuthenticationGate()
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, authenticate: { try await authentication.authenticate() }, type: .chain(.bitcoinSv))
        model.input = LocalKeystore.words.joined(separator: " ")
        model.passphrase = " public:Pass "
        await model.onSelectActionButton()
        model.isHdConfirmed = true
        let task = Task { await model.confirmHdImport() }
        await authentication.waitUntilStarted()
        model.clearSensitiveState()
        await authentication.finish()
        await task.value

        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.preview == nil)
        #expect(try await service.getWallets().isEmpty)
        #expect(try service.currentWalletId() == nil)
    }

    @Test
    func confirmedRecoveryPassesThePreviewAddressBeforeWriting() async throws {
        let keystore = LocalKeystore.mock()
        let service = HdImportServiceSpy(noHandle: .init())
        service.base = .mock(keystore: keystore)
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .chain(.bitcoinSv))
        model.input = LocalKeystore.words.joined(separator: " ")
        model.hdConfiguration.pathPreset = .compatible
        await model.onSelectActionButton()
        let firstAddress = try #require(model.preview?.account.address)
        #expect(model.hdConfiguration.originalAddress == nil)
        model.isHdConfirmed = true
        await model.confirmHdImport()

        #expect(service.expectedAddresses == [firstAddress])
        #expect(try await service.base.getWallets().isEmpty)
    }

    @Test
    func recoveryRefreshFailureKeepsTheImportedWalletAndClearsSecrets() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore, db: .mockWithChains([.bitcoinSv]))
        let setup = WalletAssetsSetupMock(error: .Store(msg: "refresh unavailable"))
        let completion = ImportCompletionSpy()
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .chain(.bitcoinSv), appStartService: setup, onComplete: { _ in completion.count += 1 })
        model.input = LocalKeystore.words.joined(separator: " ")
        await model.onSelectActionButton()
        model.isHdConfirmed = true
        await model.confirmHdImport()

        let wallet = try #require(try await service.getWallets().first)
        #expect(try service.currentWalletId() == wallet.id.id)
        #expect(await setup.wallets.count == 1)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.preview == nil)
        #expect(model.isPresentingAlertMessage != nil)
        #expect(completion.count == 0)
        #expect(!model.isBusy)
    }

    @Test
    func embeddedRecoverySettingsRemainWhenSeparateFieldsAreEmpty() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore, db: .mockWithChains([.bitcoinSv]))
        let imported = try await service.bsvHdImport(name: "Original", recoveryString: LocalKeystore.words.joined(separator: " "), accountPath: "m/44'/145'/2'", passphrase: " public:Pass ", expectedAddress: nil, legacyNonCompliant: true)
        let wallet = switch imported {
        case let .new(wallet): wallet
        case let .existing(wallet): wallet
        }
        guard case let .recovery(backup) = try await service.exportSecret(walletId: wallet.id) else {
            Issue.record("Full recovery backup required")
            return
        }
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .multicoin)
        model.input = backup
        await model.onSelectActionButton()

        #expect(model.preview?.accountPath == "m/44'/145'/2'")
        #expect(model.preview?.account.address == wallet.accounts[0].address)
        model.isHdConfirmed = true
        await model.confirmHdImport()
        #expect(try service.currentWalletId() == wallet.id)
        #expect(model.isPresentingExistingWalletName == "Original")
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(try await service.getWallets().count == 1)
    }

    @Test
    func observerScanRejectsWrongKindAndIgnoresClosedSessions() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore)
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .chain(.bitcoinSv))
        model.importType = .address
        model.onSelectScanQR()
        await model.onHandleScan("{\"kind\":\"wowpay-bsv-signed\",\"version\":1}")
        #expect(model.input.isEmpty)
        #expect(!model.isPresentingScanner)
        #expect(model.isPresentingAlertMessage != nil)
        await model.onHandleScan("late callback")
        #expect(model.input.isEmpty)
        #expect(try await service.getWallets().isEmpty)
    }

    @Test
    func observerScanAssemblesPairingBeforeOpeningPreview() async throws {
        let keystore = LocalKeystore.mock()
        let service = BsvHomeWalletService.mock(keystore: keystore)
        let pairing = try keystore.bsvHomeKeystore.bsvHdPreview(recoveryString: LocalKeystore.words.joined(separator: " "), accountPath: "m/44'/145'/0'", passphrase: nil, expectedAddress: nil, legacyNonCompliant: false)
        let frames = try keystore.bsvHomeKeystore.bsvColdQrFrames(payloadJson: pairing)
        let model = ImportWalletSceneViewModel.mock(service: service, keystore: keystore, type: .chain(.bitcoinSv))
        model.importType = .address
        model.onSelectScanQR()
        for frame in frames.reversed() {
            await model.onHandleScan(frame)
        }
        #expect(model.input == pairing)
        #expect(!model.isPresentingScanner)
        #expect(try await service.getWallets().isEmpty)
        await model.onSelectActionButton()
        #expect(model.preview?.accountPath == "m/44'/145'/0'")
        #expect(!model.isHdConfirmed)
    }
}

@MainActor
private extension ImportWalletSceneViewModel {
    static func mock(
        service: BsvHomeWalletService? = nil,
        keystore: LocalKeystore = .mock(),
        authenticate: @escaping @MainActor @Sendable () async throws -> Void = {},
        type: ImportWalletType = .chain(.ethereum),
        appStartService: any BsvHomeAppStartServiceProtocol = WalletAssetsSetupMock(),
        nameService: any BsvHomeNameServiceProtocol = BsvHomeNameServiceMock(nameRecord: .mock()),
        onComplete: (@MainActor @Sendable (ImportWalletSceneResult) -> Void)? = nil,
        purpose: WalletOnboardingPurpose = .personal,
    ) -> ImportWalletSceneViewModel {
        ImportWalletSceneViewModel(
            service: service ?? .mock(keystore: keystore),
            keystore: keystore.bsvHomeKeystore,
            authenticate: authenticate,
            appStartService: appStartService,
            preferences: .mock(),
            nameService: nameService,
            type: type,
            onComplete: onComplete,
            purpose: purpose,
        )
    }
}

@MainActor
private final class ImportCompletionSpy {
    var count = 0
}

private final class HdImportServiceSpy: BsvHomeWalletService, @unchecked Sendable {
    var base = BsvHomeWalletService.mock()
    private let lock = NSLock()
    private var addresses: [String?] = []

    var expectedAddresses: [String?] { lock.withLock { addresses } }

    override func bsvHdImportMode(kind: BsvHomeWalletImportKind, chain: BsvHomeCore.Chain?, input: String, hasOverrides: Bool) -> BsvHomeBsvHdImportMode? {
        base.bsvHdImportMode(kind: kind, chain: chain, input: input, hasOverrides: hasOverrides)
    }

    override func defaultWalletName(chain: BsvHomeCore.Chain?) async throws -> BsvHomeWalletDefaultName {
        try await base.defaultWalletName(chain: chain)
    }

    override func bsvHdImport(name _: String, recoveryString _: String, accountPath _: String?, passphrase _: String?, expectedAddress: String?, legacyNonCompliant _: Bool) async throws -> BsvHomeWalletImportResult {
        lock.withLock { addresses.append(expectedAddress) }
        throw BsvHomeServiceError.InvalidInput(msg: "stop before write")
    }
}

private final class FailingPreferencesStore: BsvHomePreferencesStore {
    func get(key _: String) -> String? {
        nil
    }

    func set(key _: String, value _: String) throws {
        throw BsvHomeServiceError.Store(msg: "session write failed")
    }

    func remove(key _: String) throws {
        throw BsvHomeServiceError.Store(msg: "session write failed")
    }

    func clear() throws {
        throw BsvHomeServiceError.Store(msg: "session write failed")
    }
}

private actor WalletAssetsSetupMock: BsvHomeAppStartServiceProtocol {
    private(set) var wallets: [BsvHomeCore.Wallet] = []
    let error: BsvHomeServiceError?

    init(error: BsvHomeServiceError? = nil) {
        self.error = error
    }

    func initializeWalletAssets(wallet: BsvHomeCore.Wallet) async throws {
        wallets.append(wallet)
        if let error {
            throw error
        }
    }

    func run() async -> [BsvHomeAppStartFailure] {
        []
    }

    func setupWallet(wallet _: BsvHomeCore.Wallet) async -> [BsvHomeAppStartFailure] {
        []
    }

    func setupWallets() async -> [BsvHomeAppStartFailure] {
        []
    }
}

private actor ImportAuthenticationGate {
    private var continuation: CheckedContinuation<Void, any Error>?
    private var started: CheckedContinuation<Void, Never>?
    private var isStarted = false

    func authenticate() async throws {
        isStarted = true
        started?.resume()
        started = nil
        try await withCheckedThrowingContinuation { continuation = $0 }
    }

    func waitUntilStarted() async {
        if isStarted {
            return
        }
        await withCheckedContinuation { started = $0 }
    }

    func finish() {
        continuation?.resume()
        continuation = nil
    }
}
