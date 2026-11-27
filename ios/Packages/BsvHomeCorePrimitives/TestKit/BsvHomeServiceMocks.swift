// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitives
import Primitives
import PrimitivesTestKit

public actor BsvHomeDeviceServiceMock: BsvHomeDeviceServiceProtocol {
    private let syncError: Error?
    private let synchronize: @Sendable () async throws -> Void
    public private(set) var synchronizeIfNeededCalls = 0

    public init(syncError: Error? = nil, synchronize: @escaping @Sendable () async throws -> Void = {}) {
        self.syncError = syncError
        self.synchronize = synchronize
    }

    public func synchronizeIfNeeded() async throws {
        synchronizeIfNeededCalls += 1
        if let syncError {
            throw syncError
        }
        try await synchronize()
    }
}

public final class BsvHomeSecureStoreMock: BsvHomeSecureStore, @unchecked Sendable {
    private var values: [String: String] = [:]

    public init() {}

    public func get(key: String) throws -> String? {
        values[key]
    }

    public func set(key: String, value: String) throws {
        values[key] = value
    }

    public func remove(key: String) throws {
        values[key] = nil
    }
}

public final class BsvHomePreferencesStoreMock: BsvHomePreferencesStore, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String: String] = [:]
    private var writeFailure: BsvHomeServiceError?

    public init() {}

    public func get(key: String) -> String? {
        lock.withLock { values[key] }
    }

    public func set(key: String, value: String) throws {
        try lock.withLock {
            if let writeFailure {
                throw writeFailure
            }
            values[key] = value
        }
    }

    public func failWrites(with error: BsvHomeServiceError?) {
        lock.withLock { writeFailure = error }
    }

    public func remove(key: String) throws {
        lock.withLock { values[key] = nil }
    }

    public func clear() throws {
        lock.withLock { values.removeAll() }
    }
}

public final class BsvHomePreferencesServiceMock: BsvHomePreferencesServiceProtocol, @unchecked Sendable {
    private let lock = NSLock()
    private var priceAlertsEnabled: Bool
    private var skippedAppVersion: String?
    private let multisigPreferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())

    public init(priceAlertsEnabled: Bool = false) {
        self.priceAlertsEnabled = priceAlertsEnabled
    }

    public func multisigSigner(origin: String, walletId: String) throws -> BsvHomeCore.WalletId? {
        try multisigPreferences.multisigSigner(origin: origin, walletId: walletId)
    }

    public func setMultisigSigner(origin: String, walletId: String, signerWalletId: BsvHomeCore.WalletId) throws {
        try multisigPreferences.setMultisigSigner(origin: origin, walletId: walletId, signerWalletId: signerWalletId)
    }

    public func setPriceAlertsEnabled(enabled: Bool) throws {
        lock.withLock { priceAlertsEnabled = enabled }
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func setCurrency(currency _: BsvHomeCore.Currency) throws {}

    public func setupCurrency(localeCurrency _: String?) throws -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func getChartPeriod() -> BsvHomeCore.ChartPeriod {
        Primitives.ChartPeriod.day.map()
    }

    public func setChartPeriod(period _: BsvHomeCore.ChartPeriod) throws {}

    public func isPushNotificationsEnabled() -> Bool {
        false
    }

    public func setPushNotificationsEnabled(enabled _: Bool) throws {}

    private var perpetualEnabled = false
    private var hideBalanceEnabled = false
    private var developerEnabled = false
    private var acceptTermsCompleted = false
    private var appearance: BsvHomeCore.Appearance = .system

    public func isPerpetualEnabled() -> Bool {
        perpetualEnabled
    }

    public func setPerpetualEnabled(enabled: Bool) throws {
        perpetualEnabled = enabled
    }

    public func showPerpetuals(walletType _: BsvHomeCore.WalletType, chains _: [BsvHomeCore.Chain]) -> Bool {
        perpetualEnabled
    }

    public var collectionsShown = true

    public func showCollections(walletType _: BsvHomeCore.WalletType, chains _: [BsvHomeCore.Chain]) -> Bool {
        collectionsShown
    }

    public func isHideBalanceEnabled() -> Bool {
        hideBalanceEnabled
    }

    public func setHideBalanceEnabled(enabled: Bool) throws {
        hideBalanceEnabled = enabled
    }

    public func isDeveloperEnabled() -> Bool {
        developerEnabled
    }

    public func setDeveloperEnabled(enabled: Bool) throws {
        developerEnabled = enabled
    }

    public func isAcceptTermsCompleted() -> Bool {
        acceptTermsCompleted
    }

    public func setAcceptTermsCompleted() throws {
        acceptTermsCompleted = true
    }

    public func getAppearance() -> BsvHomeCore.Appearance {
        appearance
    }

    public func setAppearance(appearance: BsvHomeCore.Appearance) throws {
        self.appearance = appearance
    }

    public func getPerpetualLeverage() -> UInt8 {
        5
    }

    public func setPerpetualLeverage(leverage _: UInt8) throws {}

    public func getPerpetualTakeProfitPercent() -> UInt8 {
        0
    }

    public func setPerpetualTakeProfitPercent(percent _: UInt8) throws {}

    public func getPerpetualStopLossPercent() -> UInt8 {
        0
    }

    public func setPerpetualStopLossPercent(percent _: UInt8) throws {}

    public func incrementLaunchesCount() throws -> UInt32 {
        1
    }

    public func shouldRequestReview() -> Bool {
        false
    }

    public func setRateApplicationShown() throws {}

    public func shouldAskNotifications() -> Bool {
        false
    }

    public func setNotificationsAsked() throws {}

    public func clear() throws {}
}

public final class BsvHomePriceAlertServiceMock: BsvHomePriceAlertServiceProtocol, @unchecked Sendable {
    private let lock = NSLock()
    private var enabled: Bool

    public init(enabled: Bool = false) {
        self.enabled = enabled
    }

    public func isEnabled() -> Bool {
        lock.withLock { enabled }
    }

    public func setEnabled(enabled: Bool) async throws {
        lock.withLock { self.enabled = enabled }
    }

    public func sync(assetId _: String?) async throws {}

    public func enablePriceAlert(alert _: BsvHomeCore.PriceAlert) async throws {
        lock.withLock { enabled = true }
    }

    public func deletePriceAlerts(alerts _: [BsvHomeCore.PriceAlert]) async throws {}

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func setAutoAlert(assetId _: BsvHomeCore.AssetId, enabled isEnabled: Bool) async throws {
        lock.withLock { enabled = isEnabled }
    }

    public func priceAlertId(alert: BsvHomeCore.PriceAlert) -> String {
        alert.map().id
    }
}

public final class StubAlienProvider: AlienProvider, @unchecked Sendable {
    public init() {}

    public func request(target _: AlienTarget) async throws -> AlienResponse {
        throw AnyError("StubAlienProvider does not perform requests")
    }
}

private func contactService(environment: BsvHomeRuntimeEnvironment = .production) -> BsvHomeContactService {
    BsvHomeContactService.newWithEnvironment(
        store: BsvHomeContactStoreMock(),
        addressStore: BsvHomeAddressStoreMock(),
        files: BsvHomeFileStoreMock(),
        environment: environment,
    )
}

public final class BsvHomeManageContactServiceMock: BsvHomeManageContactServiceProtocol, @unchecked Sendable {
    private let service: BsvHomeManageContactService

    public init(environment: BsvHomeRuntimeEnvironment = .production) {
        service = BsvHomeManageContactService(
            contacts: contactService(environment: environment),
            addresses: BsvHomeAddressService(),
            payments: BsvHomePaymentService.mock(),
        )
    }

    public func scannedAddress(input: String) -> BsvHomeContactScannedAddress {
        service.scannedAddress(input: input)
    }

    public func defaultChain() -> BsvHomeCore.Chain {
        service.defaultChain()
    }

    public func saveContact(input: BsvHomeContactInput) async throws -> BsvHomeCore.Contact {
        try await service.saveContact(input: input)
    }

    public func formatAddress(address: String, chain: BsvHomeCore.Chain, style: BsvHomeAddressFormatStyle) -> String {
        service.formatAddress(address: address, chain: chain, style: style)
    }
}

public final class BsvHomeContactStoreMock: BsvHomeContactStore, @unchecked Sendable {
    public init() {}

    public func getAddresses(contactId _: String) async throws -> [BsvHomeCore.ContactAddress] {
        []
    }

    public func saveContact(contact _: BsvHomeCore.Contact, addresses _: [BsvHomeCore.ContactAddress]) async throws {}

    public func updateContact(contact _: BsvHomeCore.Contact, addresses _: [BsvHomeCore.ContactAddress], deleteAddressIds _: [String]) async throws {}

    public func deleteContact(contactId _: String) async throws {}
}

public final class BsvHomeAddressStoreMock: BsvHomeAddressStore, @unchecked Sendable {
    public init() {}

    public func getAddressName(chain _: BsvHomeCore.Chain, address _: String) throws -> BsvHomeCore.AddressName? {
        nil
    }

    public func saveAddressNames(names _: [BsvHomeCore.AddressName]) async throws {}

    public func deleteAddressNames(names _: [BsvHomeCore.AddressName]) async throws {}
}

public final class BsvHomeFileStoreMock: BsvHomeFileStore, @unchecked Sendable {
    public init() {}

    public func saveFile(data _: Data, extension _: String) throws -> String {
        ""
    }

    public func saveNamedFile(data _: Data, fileName: String) throws -> String {
        fileName
    }

    public func exists(fileName _: String) -> Bool {
        false
    }

    public func path(fileName: String) -> String {
        fileName
    }

    public func remove(fileName _: String) throws {}
}

public final class BsvHomeStreamServiceMock: BsvHomeStreamServiceProtocol, @unchecked Sendable {
    private let prepare: @Sendable () async throws -> Bool
    private let onConnected: @Sendable () async throws -> Void
    private let onDisconnected: @Sendable () async -> Void
    private let onEvent: @Sendable (String) async throws -> Void

    public init(
        prepare: @escaping @Sendable () async throws -> Bool = { true },
        onConnected: @escaping @Sendable () async throws -> Void = {},
        onDisconnected: @escaping @Sendable () async -> Void = {},
        onEvent: @escaping @Sendable (String) async throws -> Void = { _ in },
    ) {
        self.prepare = prepare
        self.onConnected = onConnected
        self.onDisconnected = onDisconnected
        self.onEvent = onEvent
    }

    public func prepareConnection() async throws -> Bool {
        try await prepare()
    }

    public func connected() async throws {
        try await onConnected()
    }

    public func disconnected() async {
        await onDisconnected()
    }

    public func handle(event: String) async throws {
        try await onEvent(event)
    }
}

public final class BsvHomeAmountServiceMock: BsvHomeAmountServiceProtocol, @unchecked Sendable {
    private let builder: any BsvHomeAmountServiceProtocol

    public init(builder: any BsvHomeAmountServiceProtocol) {
        self.builder = builder
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func stakeTransferData(asset: BsvHomeCore.Asset, stakeType: BsvHomeCore.StakeType, value: BsvHomeCore.BsvHomeBigInt, useMaxAmount: Bool) -> BsvHomeTransferData {
        builder.stakeTransferData(asset: asset, stakeType: stakeType, value: value, useMaxAmount: useMaxAmount)
    }

    public func perpetualTransferData(action: BsvHomePerpetualPositionAction, value: BsvHomeCore.BsvHomeBigInt, useMaxAmount: Bool, leverage: UInt8, takeProfit: Double?, stopLoss: Double?) -> BsvHomeTransferData {
        builder.perpetualTransferData(action: action, value: value, useMaxAmount: useMaxAmount, leverage: leverage, takeProfit: takeProfit, stopLoss: stopLoss)
    }

    public func earnTransferData(asset _: BsvHomeCore.Asset, earnType _: BsvHomeCore.EarnType, value _: BsvHomeCore.BsvHomeBigInt, useMaxAmount _: Bool) async throws -> BsvHomeTransferData {
        throw AnyError("not stubbed")
    }

    public func perpetualLeverage(maxLeverage: UInt8) -> UInt8 {
        min(5, maxLeverage)
    }

    public func perpetualAmountType(action: BsvHomePerpetualPositionAction, leverage: UInt8) -> BsvHomeAmountType {
        builder.perpetualAmountType(action: action, leverage: leverage)
    }

    public func stakeValidatorSelection(chain: BsvHomeCore.Chain, input: BsvHomeStakeAmountInput) -> BsvHomeStakeValidatorSelection {
        builder.stakeValidatorSelection(chain: chain, input: input)
    }

    public func earnAmountType(earnType: BsvHomeCore.EarnType) -> BsvHomeAmountType {
        builder.earnAmountType(earnType: earnType)
    }

    public func transferData(asset: BsvHomeCore.Asset, transfer: BsvHomeAmountTransfer, value: BsvHomeCore.BsvHomeBigInt, useMaxAmount: Bool) async throws -> BsvHomeTransferData {
        try await builder.transferData(asset: asset, transfer: transfer, value: value, useMaxAmount: useMaxAmount)
    }

    public func perpetualAutoclose(price _: Double, direction _: BsvHomeCore.PerpetualDirection, leverage _: UInt8) -> BsvHomePerpetualAutoclose {
        BsvHomePerpetualAutoclose(takeProfit: nil, stopLoss: nil)
    }
}

public final class BsvHomeFiatQuoteServiceMock: BsvHomeFiatQuoteServiceProtocol, @unchecked Sendable {
    private let quotes: [BsvHomeCore.FiatQuote]
    private let check: @Sendable (BsvHomeCore.FiatQuote?) -> BsvHomeFiatAmountCheck

    public init(quotes: [BsvHomeCore.FiatQuote] = [], check: @escaping @Sendable (BsvHomeCore.FiatQuote?) -> BsvHomeFiatAmountCheck = { _ in .valid }) {
        self.quotes = quotes
        self.check = check
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func suggestedAmounts() -> [Int32] {
        [100, 250]
    }

    private func defaultAmount(quoteType: BsvHomeCore.FiatQuoteType) -> UInt32 {
        quoteType.map() == .sell ? 100 : 50
    }

    public func newSession(quoteType: BsvHomeCore.FiatQuoteType, amount: UInt32?) -> BsvHomeFiatSession {
        let operation = { (type: BsvHomeCore.FiatQuoteType) -> BsvHomeFiatOperation in
            let value = (type == quoteType ? amount : nil) ?? self.defaultAmount(quoteType: type)
            return BsvHomeFiatOperation(quoteType: type, amount: String(value), quotes: [], selectedProvider: nil, phase: .loading(amount: Double(value)))
        }
        return BsvHomeFiatSession(quoteType: quoteType, buy: operation(.buy), sell: operation(.sell), available: 0)
    }

    public func randomAmount() -> UInt32 {
        50
    }

    public func amountCheck(quoteType _: BsvHomeCore.FiatQuoteType, amount _: Double, quote: BsvHomeCore.FiatQuote?, available _: BsvHomeBigUint) -> BsvHomeFiatAmountCheck {
        check(quote)
    }

    public func quoteDebounceMilliseconds() -> UInt64 {
        250
    }

    public func quoteRefreshIntervalMilliseconds() -> UInt64 {
        300_000
    }

    public func syncTransactions() async throws {}

    public func quotes(quoteType _: BsvHomeCore.FiatQuoteType, assetId _: BsvHomeCore.AssetId, amount _: Double) async throws -> [BsvHomeCore.FiatQuote] {
        quotes
    }

    public func quoteUrl(assetId _: BsvHomeCore.AssetId, quoteId _: String) async throws -> BsvHomeCore.FiatQuoteUrl {
        throw AnyError("not stubbed")
    }
}

public extension BsvHomePaymentService {
    static func mock() -> BsvHomePaymentService {
        BsvHomePaymentService(provider: StubAlienProvider())
    }
}

public extension BsvHomeNameService {
    static func mock() -> BsvHomeNameService {
        BsvHomeNameService(
            api: BsvHomeDeviceApiClient(
                provider: StubAlienProvider(),
                deviceKey: BsvHomeDeviceKeyService(store: BsvHomeSecureStoreMock()),
            ),
            store: BsvHomeAddressStoreMock(),
        )
    }
}

public final class BsvHomeNameServiceMock: BsvHomeNameServiceProtocol, @unchecked Sendable {
    private let rules = BsvHomeNameService.mock()
    private let addressNames: [Primitives.AddressName]
    private let nameRecord: Primitives.NameRecord?
    private let error: Error?
    public private(set) var requestedNames: [String] = []

    public init(addressNames: [Primitives.AddressName] = [], nameRecord: Primitives.NameRecord? = nil, error: Error? = nil) {
        self.addressNames = addressNames
        self.nameRecord = nameRecord
        self.error = error
    }

    public func getNameRecord(name: String, chain _: String) async throws -> BsvHomeNameRecordState {
        requestedNames.append(name)
        if let error {
            throw error
        }
        return nameRecord.map { .complete(record: $0.map()) } ?? .error
    }

    public func isNameSupported(name: String) -> Bool {
        name.split(separator: ".").count >= 2
    }

    public func nameRecordDebounceMilliseconds() -> UInt64 {
        0
    }

    public func validateRecipient(chain: BsvHomeCore.Chain, input: String, state: BsvHomeNameRecordState) -> BsvHomeRecipientValidation {
        rules.validateRecipient(chain: chain, input: input, state: state)
    }

    public func recipient(chain: BsvHomeCore.Chain, input: String, state: BsvHomeNameRecordState, memo: String?, references: [String]) throws -> BsvHomeRecipient {
        try rules.recipient(chain: chain, input: input, state: state, memo: memo, references: references)
    }
}

public final class BsvHomePortfolioServiceMock: BsvHomePortfolioServiceProtocol, @unchecked Sendable {
    private let allTimeHigh: Primitives.ChartValuePercentage?
    private let allTimeLow: Primitives.ChartValuePercentage?

    public init(allTimeHigh: Primitives.ChartValuePercentage? = nil, allTimeLow: Primitives.ChartValuePercentage? = nil) {
        self.allTimeHigh = allTimeHigh
        self.allTimeLow = allTimeLow
    }

    public func portfolioData(wallet _: BsvHomeCore.Wallet, portfolioType _: BsvHomeCore.PortfolioType, period _: BsvHomeCore.ChartPeriod) async throws -> BsvHomeCore.PortfolioData {
        BsvHomeCore.PortfolioData(
            charts: [BsvHomeCore.PortfolioChartData(chartType: .value, values: [])],
            statistics: [allTimeHigh.map { .allTimeHigh(value: $0.map()) }, allTimeLow.map { .allTimeLow(value: $0.map()) }].compactMap(\.self),
            availablePeriods: [.day, .week, .month, .year, .all],
        )
    }
}

public final class BsvHomeStakeServiceMock: BsvHomeStakeServiceProtocol, @unchecked Sendable {
    private let rewardsShown: Bool
    private let claimable: Bool
    private let explorerAddress: String?
    private let actions: [BsvHomeCore.BsvHomeDelegationAction]
    private let validators: [BsvHomeCore.DelegationValidator]
    private let lockTime: UInt64
    private let minStake: BsvHomeCore.BsvHomeBigInt
    private let changesAmountOnUnstake: Bool
    private let freezes: Bool
    private let wholeAmounts: Bool

    public init(
        rewardsShown: Bool = false,
        claimable: Bool = false,
        explorerAddress: String? = nil,
        actions: [BsvHomeCore.BsvHomeDelegationAction] = [],
        validators: [BsvHomeCore.DelegationValidator] = [],
        lockTime: UInt64 = 0,
        minStake: BsvHomeCore.BsvHomeBigInt = 0,
        changesAmountOnUnstake: Bool = false,
        freezes: Bool = false,
        wholeAmounts: Bool = false,
    ) {
        self.rewardsShown = rewardsShown
        self.claimable = claimable
        self.explorerAddress = explorerAddress
        self.actions = actions
        self.validators = validators
        self.lockTime = lockTime
        self.minStake = minStake
        self.changesAmountOnUnstake = changesAmountOnUnstake
        self.freezes = freezes
        self.wholeAmounts = wholeAmounts
    }

    public func lockTimeSeconds(chain _: BsvHomeCore.Chain) -> UInt64 {
        lockTime
    }

    public func minStakeAmount(chain _: BsvHomeCore.Chain) -> BsvHomeCore.BsvHomeBigInt {
        minStake
    }

    public func canChangeAmountOnUnstake(chain _: BsvHomeCore.Chain) -> Bool {
        changesAmountOnUnstake
    }

    public func usesFreeze(chain _: BsvHomeCore.Chain) -> Bool {
        freezes
    }

    public func stakeTransferData(asset: BsvHomeCore.Asset, stakeType: BsvHomeCore.StakeType, value: BsvHomeCore.BsvHomeBigInt, useMaxAmount: Bool) -> BsvHomeTransferData {
        BsvHomeTransferData(inputType: .stake(asset: asset, stakeType: stakeType), recipient: BsvHomeRecipient(address: ""), value: value, useMaxAmount: useMaxAmount)
    }

    public func delegationDestination(walletType _: BsvHomeCore.WalletType, asset _: BsvHomeCore.Asset, delegation _: BsvHomeCore.Delegation) -> BsvHomeDelegationDestination {
        .details
    }

    public func delegationActions(walletType _: BsvHomeCore.WalletType, delegation _: BsvHomeCore.Delegation) -> [BsvHomeCore.BsvHomeDelegationAction] {
        actions
    }

    public func canClaimDelegationRewards(walletType _: BsvHomeCore.WalletType, delegation _: BsvHomeCore.Delegation) -> Bool {
        claimable
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func validatorUrl(validator _: BsvHomeCore.DelegationValidator) -> BsvHomeCore.BlockExplorerLink? {
        explorerAddress.map { BsvHomeCore.BlockExplorerLink(name: "MockExplorer", link: "https://explorer.mock/validator/\($0)") }
    }

    public func showsRewards(delegation _: BsvHomeCore.DelegationBase) -> Bool {
        rewardsShown
    }

    public func stakeActions(walletType _: BsvHomeCore.WalletType, chain _: BsvHomeCore.Chain, hasValidators: Bool, balance _: BsvHomeAssetBalance, delegations _: [BsvHomeCore.Delegation]) -> [BsvHomeStakeActionItem] {
        [BsvHomeStakeActionItem(action: .stake, isEnabled: hasValidators, requiresFrozenBalance: false)]
    }

    public func claimRewards(chain _: BsvHomeCore.Chain, delegations: [BsvHomeCore.Delegation]) -> BsvHomeClaimRewards {
        BsvHomeClaimRewards(value: 0, destination: .amount(delegations: delegations))
    }

    public func selectableValidators(validators _: [BsvHomeCore.DelegationValidator]) -> [BsvHomeCore.DelegationValidator] {
        validators
    }

    public func sync(chain _: BsvHomeCore.Chain) async throws {}

    public func syncEarn(assetId _: BsvHomeCore.AssetId) async throws {}
}

public final class BsvHomeTransactionStateServiceMock: BsvHomeTransactionStateServiceProtocol, @unchecked Sendable {
    private let store: (any BsvHomeTransactionStateStore)?
    private let notificationAsset: BsvHomeCore.Asset?
    private let generation: @Sendable () -> UInt64
    private let onTrackWithGeneration: @Sendable (BsvHomeCore.WalletId, [BsvHomeCore.Transaction], UInt64) async throws -> Void
    private let onPauseTracking: @Sendable () -> Void
    private let onResumeTracking: @Sendable () -> Void
    private let onStopTracking: @Sendable () -> Void

    public init(
        store: (any BsvHomeTransactionStateStore)? = nil,
        notificationAsset: BsvHomeCore.Asset? = nil,
        trackingGeneration: @escaping @Sendable () -> UInt64 = { 0 },
        onTrackWithGeneration: @escaping @Sendable (BsvHomeCore.WalletId, [BsvHomeCore.Transaction], UInt64) async throws -> Void = { _, _, _ in },
        onPauseTracking: @escaping @Sendable () -> Void = {},
        onResumeTracking: @escaping @Sendable () -> Void = {},
        onStopTracking: @escaping @Sendable () -> Void = {},
    ) {
        self.store = store
        self.notificationAsset = notificationAsset
        generation = trackingGeneration
        self.onTrackWithGeneration = onTrackWithGeneration
        self.onPauseTracking = onPauseTracking
        self.onResumeTracking = onResumeTracking
        self.onStopTracking = onStopTracking
    }

    public func trackPending() async throws {}

    public func track(walletId _: BsvHomeCore.WalletId, transactions _: [BsvHomeCore.Transaction]) async throws {}

    public func trackingGeneration() -> UInt64 {
        generation()
    }

    public func trackWithGeneration(walletId: BsvHomeCore.WalletId, transactions: [BsvHomeCore.Transaction], generation: UInt64) async throws {
        try await onTrackWithGeneration(walletId, transactions, generation)
    }

    public func pauseTracking() {
        onPauseTracking()
    }

    public func resumeTracking() {
        onResumeTracking()
    }

    public func stopTracking() {
        onStopTracking()
    }

    public func addNotificationTransaction(wallet _: BsvHomeCore.Wallet, assetId _: BsvHomeCore.AssetId, transaction: BsvHomeCore.Transaction) async throws -> BsvHomeCore.Asset? {
        if let store {
            try await store.addTransactions(walletId: "", transactions: [transaction])
        }
        return notificationAsset
    }
}

public final class BsvHomeBalanceServiceMock: BsvHomeBalanceServiceProtocol, @unchecked Sendable {
    private let onUpdate: @Sendable (String, [BsvHomeCore.AssetId]) async -> Void
    private let onSetAssetsEnabled: (@Sendable (String, [BsvHomeCore.AssetId], Bool) async throws -> Void)?
    private let onSetAssetPinned: (@Sendable (String, BsvHomeCore.AssetId, Bool) async throws -> Void)?
    private let assetBalances: [BsvHomeAssetBalance]

    public init(
        onUpdate: @escaping @Sendable (String, [BsvHomeCore.AssetId]) async -> Void = { _, _ in },
        onSetAssetsEnabled: (@Sendable (String, [BsvHomeCore.AssetId], Bool) async throws -> Void)? = nil,
        onSetAssetPinned: (@Sendable (String, BsvHomeCore.AssetId, Bool) async throws -> Void)? = nil,
        assetBalances: [BsvHomeAssetBalance] = [],
    ) {
        self.onUpdate = onUpdate
        self.onSetAssetsEnabled = onSetAssetsEnabled
        self.onSetAssetPinned = onSetAssetPinned
        self.assetBalances = assetBalances
    }

    public func balances(walletId _: String, assetIds: [BsvHomeCore.AssetId]) throws -> [BsvHomeAssetBalance] {
        assetBalances.filter { assetIds.contains($0.assetId) }
    }

    public func update(walletId: String, assetIds: [BsvHomeCore.AssetId]) async throws {
        await onUpdate(walletId, assetIds)
    }

    public func setAssetsEnabled(walletId: String, assetIds: [BsvHomeCore.AssetId], enabled: Bool) async throws {
        try await onSetAssetsEnabled?(walletId, assetIds, enabled)
    }

    public func setAssetPinned(walletId: String, assetId: BsvHomeCore.AssetId, pinned: Bool) async throws {
        try await onSetAssetPinned?(walletId, assetId, pinned)
    }
}

public extension BsvHomeBalanceServiceProtocol where Self == BsvHomeBalanceServiceMock {
    static func mock(
        onSetAssetsEnabled: (@Sendable (String, [BsvHomeCore.AssetId], Bool) async throws -> Void)? = nil,
        onSetAssetPinned: (@Sendable (String, BsvHomeCore.AssetId, Bool) async throws -> Void)? = nil,
    ) -> BsvHomeBalanceServiceMock {
        BsvHomeBalanceServiceMock(onSetAssetsEnabled: onSetAssetsEnabled, onSetAssetPinned: onSetAssetPinned)
    }
}

public final class BsvHomePerpetualServiceMock: BsvHomePerpetualServiceProtocol, @unchecked Sendable {
    public var autocloseSummary: BsvHomeAutocloseSummary?
    public var isPerpetualEnabled = true
    public var connects = true
    public private(set) var syncMarketsCount = 0
    public private(set) var syncPositionsCount = 0
    public private(set) var clearMarketsCount = 0
    public var connectionFailures = 0
    private var updatedAt: Int64?

    public init(marketsUpdatedAt: Int64? = nil) {
        updatedAt = marketsUpdatedAt
    }

    public func marketsUpdatedAt() throws -> Int64? {
        updatedAt
    }

    public func autocloseSummary(data _: BsvHomeCore.PerpetualModifyConfirmData) -> BsvHomeAutocloseSummary? {
        autocloseSummary
    }

    public func syncEnablement(wallet: BsvHomeCore.Wallet?, trigger: BsvHomeCore.BsvHomeMarketsRefreshTrigger) async throws -> Bool {
        if isPerpetualEnabled {
            _ = try await syncMarketsIfNeeded(chain: "hypercore", trigger: trigger)
        } else {
            try await clearMarkets()
        }
        return shouldConnectPerpetuals(wallet: wallet)
    }

    public func shouldConnectPerpetuals(wallet _: BsvHomeCore.Wallet?) -> Bool {
        isPerpetualEnabled && connects
    }

    private func syncMarketsIfNeeded(chain: BsvHomeCore.Chain, trigger: BsvHomeCore.BsvHomeMarketsRefreshTrigger) async throws -> Bool {
        if trigger == .scheduled, updatedAt != nil {
            return false
        }
        try await syncMarkets(chain: chain)
        return true
    }

    private func syncMarkets(chain _: BsvHomeCore.Chain) async throws {
        syncMarketsCount += 1
        updatedAt = Int64(Date().timeIntervalSince1970)
    }

    public func clearMarkets() async throws {
        clearMarketsCount += 1
        updatedAt = nil
    }

    private func syncCurrentPositions() async throws {
        syncPositionsCount += 1
    }

    public func refresh(trigger: BsvHomeCore.BsvHomeMarketsRefreshTrigger) async -> [BsvHomeCore.BsvHomePerpetualRefreshFailure] {
        try? await syncCurrentPositions()
        _ = try? await syncMarketsIfNeeded(chain: "hypercore", trigger: trigger)
        return []
    }

    public func connection(wallet: BsvHomeCore.Wallet) async throws -> BsvHomeCore.BsvHomePerpetualConnection? {
        if connectionFailures > 0 {
            connectionFailures -= 1
            throw AnyError("connection unavailable")
        }
        guard let account = wallet.map().hyperliquidAccount else { return nil }
        return try BsvHomeCore.BsvHomePerpetualConnection(
            address: account.address,
            mode: Primitives.PerpetualAccountMode.standard.map(),
        )
    }

    public func setPinned(perpetualId _: String, pinned _: Bool) async throws {}
}

public extension BsvHomeExplorerService {
    static func mock() -> BsvHomeExplorerService {
        BsvHomeExplorerService(preferences: BsvHomePreferencesService(store: BsvHomePreferencesStoreMock()))
    }
}

public extension BsvHomeTransactionDetailsService {
    static func mock() -> BsvHomeTransactionDetailsService {
        let preferences = BsvHomePreferencesService(store: BsvHomePreferencesStoreMock())
        return BsvHomeTransactionDetailsService(explorer: BsvHomeExplorerService(preferences: preferences), preferences: preferences)
    }
}

public final class BsvHomeWalletHomeServiceMock: BsvHomeWalletHomeServiceProtocol, @unchecked Sendable {
    public private(set) var handledActions: [BsvHomeBannerAction] = []
    public private(set) var pinned: [(assetId: BsvHomeCore.AssetId, pinned: Bool)] = []
    public private(set) var enabled: [(assetIds: [BsvHomeCore.AssetId], enabled: Bool)] = []
    public var showsLoading = false
    public var developmentIds: [BsvHomeCore.AssetId] = []
    public var availableChainIds: [BsvHomeCore.Chain] = Primitives.Chain.productionCases.map(\.rawValue)
    private let refreshHandler: @Sendable () async throws -> Void

    public init(refreshHandler: @escaping @Sendable () async throws -> Void = {}) {
        self.refreshHandler = refreshHandler
    }

    public func developmentAssetIds(wallet _: BsvHomeCore.Wallet) -> [BsvHomeCore.AssetId] {
        developmentIds
    }

    public func availableChains() -> [BsvHomeCore.Chain] {
        availableChainIds
    }

    public func convertTotalValue(value: Double, currency: BsvHomeCore.Currency, rates: [BsvHomeCore.FiatRate]) -> Double? {
        let rate = currency == "USD" ? 1 : rates.first { $0.symbol == currency }?.rate
        return rate.map { value * $0 }
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        Primitives.Currency.usd.rawValue
    }

    public func viewState(wallet: BsvHomeCore.Wallet, balances: [BsvHomeCore.AssetFiatValue], perpetual: BsvHomeCore.PerpetualBalance?, banners: [BsvHomeCore.Banner], isWalletEmpty: Bool) -> BsvHomeWalletHomeViewState {
        let value = balances.reduce(0.0) { $0 + $1.amount * $1.price } + (perpetual.map { $0.available + $0.reserved } ?? 0)
        let total = BsvHomeCore.TotalFiatValue(value: value, pnlAmount: 0, pnlPercentage: 0)
        let isEnabled = !banners.contains { $0.event == .accountBlockedMultiSignature }
        let context = BsvHomeBannerContext(
            wallet: wallet,
            asset: nil,
            isStakeable: false,
            hasStakeBalance: false,
            hasAvailableBalance: false,
            isAssetActivated: true,
            assetRankScore: nil,
            isWalletEmpty: isWalletEmpty,
        )
        return BsvHomeWalletHomeViewState(
            totalValue: total,
            showsPnl: total.value > 0 && total.pnlAmount != 0,
            headerActions: .buttons(buttons: (wallet.walletType == .view && BsvHomeRuntimeEnvironment.current == .wowPay ? [.receive] : [BsvHomeHeaderButtonKind.send, .receive, .buy]).map { BsvHomeHeaderButton(kind: $0, isEnabled: isEnabled) }),
            showCollections: false,
            visibleBanners: context.visibleBanners(stored: banners),
        )
    }

    public func updateBalances(assetIds _: [BsvHomeCore.AssetId]) async throws {}

    public func showsInitialLoading() throws -> Bool {
        showsLoading
    }

    public func refresh() async throws {
        try await refreshHandler()
    }

    public func setAssetPinned(assetId: BsvHomeCore.AssetId, pinned isPinned: Bool) async throws {
        pinned.append((assetId, isPinned))
    }

    public func setAssetsEnabled(assetIds: [BsvHomeCore.AssetId], enabled isEnabled: Bool) async throws {
        enabled.append((assetIds, isEnabled))
    }

    public func bannerContent(event _: BsvHomeCore.BannerEvent, asset _: BsvHomeCore.Asset?) -> BsvHomeBannerContent {
        BsvHomeBannerContent(icon: .none, title: .none, description: .none, link: .none)
    }

    public func applyBannerAction(key _: BsvHomeBannerKey, action: BsvHomeBannerAction) async throws {
        handledActions.append(action)
    }
}

public final class BsvHomeCurrencyServiceMock: BsvHomeCurrencyServiceProtocol, @unchecked Sendable {
    public private(set) var setCurrencies: [BsvHomeCore.Currency] = []
    private let error: Error?
    private let flag: String
    private let preferences: (any BsvHomePreferencesServiceProtocol)?

    public init(flag: String = "🇺🇸", error: Error? = nil, preferences: (any BsvHomePreferencesServiceProtocol)? = nil) {
        self.flag = flag
        self.error = error
        self.preferences = preferences
    }

    public func getCurrency() -> BsvHomeCore.Currency {
        preferences?.getCurrency() ?? setCurrencies.last ?? Primitives.Currency.usd.rawValue
    }

    public func setCurrency(currency: BsvHomeCore.Currency) async throws {
        if let error {
            throw error
        }
        try preferences?.setCurrency(currency: currency)
        setCurrencies.append(currency)
    }

    public func currencies(locale _: BsvHomeCore.Currency?) -> BsvHomeCurrencies {
        let selected = BsvHomeCurrencyRow(currency: getCurrency(), flag: flag)
        return BsvHomeCurrencies(selected: selected, recommended: [selected], other: [])
    }
}

public final class BsvHomeBannerServiceMock: BsvHomeBannerServiceProtocol, @unchecked Sendable {
    public private(set) var closedKeys: [BsvHomeBannerKey] = []
    public private(set) var handledActions: [BsvHomeBannerAction] = []

    public init() {}

    public func close(key: BsvHomeBannerKey) async throws {
        closedKeys.append(key)
    }

    public func applyAction(key _: BsvHomeBannerKey, action: BsvHomeBannerAction) async throws {
        handledActions.append(action)
    }

    public func setup() async throws {}

    public func setupWallet(wallet _: BsvHomeCore.Wallet) async throws {}

    public func bannerContent(event _: BsvHomeCore.BannerEvent, asset _: BsvHomeCore.Asset?) -> BsvHomeBannerContent {
        BsvHomeBannerContent(icon: .none, title: .none, description: .none, link: .none)
    }
}

public final class BsvHomeSearchServiceMock: BsvHomeSearchServiceProtocol, @unchecked Sendable {
    private let assets: [Primitives.AssetBasic]

    public init(assets: [Primitives.AssetBasic] = []) {
        self.assets = assets
    }

    public func search(wallet _: BsvHomeCore.Wallet, query _: String, scope _: BsvHomeSearchScope) async throws -> Bool {
        !assets.isEmpty
    }

    public func searchAssets(wallet _: BsvHomeCore.Wallet, query _: String) async throws -> [BsvHomeCore.AssetBasic] {
        assets.map { $0.map() }
    }
}

public final class BsvHomeAvatarServiceMock: BsvHomeAvatarServiceProtocol, @unchecked Sendable {
    public init() {}

    public func setImage(walletId _: BsvHomeCore.WalletId, image _: Data) async throws {}

    public func setImageUrl(walletId _: BsvHomeCore.WalletId, url _: String) async throws {}

    public func removeImage(walletId _: BsvHomeCore.WalletId) async throws {}
}

public final class BsvHomeWalletPreferencesStoreMock: BsvHomeWalletPreferencesStore, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String: String] = [:]

    public init() {}

    public func get(walletId: BsvHomeCore.WalletId, key: String) -> String? {
        lock.withLock { values["\(walletId):\(key)"] }
    }

    public func set(walletId: BsvHomeCore.WalletId, key: String, value: String) throws {
        lock.withLock { values["\(walletId):\(key)"] = value }
    }

    public func deletePreferences(walletId: BsvHomeCore.WalletId) throws {
        lock.withLock { values = values.filter { !$0.key.hasPrefix("\(walletId):") } }
    }
}

public extension BsvHomeWalletPreferencesService {
    static func mock() -> BsvHomeWalletPreferencesService {
        BsvHomeWalletPreferencesService(store: BsvHomeWalletPreferencesStoreMock())
    }
}

public extension BsvHomeCore.BsvHomeFeeAsset {
    static func mock(
        asset: Primitives.Asset,
        balance: BsvHomeCore.BsvHomeAssetBalance? = nil,
        price: BsvHomeCore.AssetPrice? = nil,
    ) -> BsvHomeCore.BsvHomeFeeAsset {
        BsvHomeCore.BsvHomeFeeAsset(
            asset: asset.map(),
            balance: balance ?? .mock(assetId: asset.id.identifier),
            price: price,
        )
    }
}

private extension Primitives.Wallet {
    var supportsPerpetuals: Bool {
        isMultiCoins && hyperliquidAccount != nil
    }

    var hyperliquidAccount: Primitives.Account? {
        accounts.first { $0.chain == .hyperCore }
    }
}
