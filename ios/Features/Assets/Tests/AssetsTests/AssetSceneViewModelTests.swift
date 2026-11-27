// Copyright (c). Gem Wallet. All rights reserved.

@testable import Assets
import BigInt
import protocol BsvHomeCore.BsvHomeAssetDetailsServiceProtocol
import struct BsvHomeCore.BsvHomeAssetRefreshFailure
import struct BsvHomeCore.BsvHomeBsvAddressRecord
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import enum BsvHomeCore.BsvHomeBsvAddressStatus
import class BsvHomeCore.BsvHomeDeeplinkService
import struct BsvHomeCore.BsvHomeHeaderButton
import protocol BsvHomeCore.BsvHomePriceAlertServiceProtocol
import struct BsvHomeCore.BsvHomeSwapPairSuggestion
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
@testable import Store
import SwiftUI
import Testing

@MainActor
struct AssetSceneViewModelTests {
    @Test(arguments: [Chain.bitcoinSv.asset, .mockTronUSDT()], [true, false])
    func historyFailurePreservesCachedRecords(asset: Asset, hasCache: Bool) async {
        let message = "InvalidJSON: backend response"
        let service = BsvHomeAssetDetailsServiceMock(refreshFailures: [.init(step: .syncTransactions, message: message)])
        let model = AssetSceneViewModel.mock(.mock(asset: asset), service: service)
        let sections = hasCache ? TransactionViewModel.sections([.mock(asset: asset)]) : []
        model.transactionsQuery.value = sections

        await model.load()

        #expect(model.transactionsState.isError)
        #expect(model.transactionSections.flatMap(\.values) == sections.flatMap(\.values))
        #expect(model.isPresentingToastMessage != nil)
        #expect(model.isPresentingToastMessage?.title != message)

        service.refreshFailures = []
        await model.load()

        guard case .data = model.transactionsState else {
            Issue.record("Expected successful history refresh")
            return
        }
        #expect(model.transactionSections.flatMap(\.values) == sections.flatMap(\.values))
    }

    @Test
    func historyLoadingRetainsCachedRecords() async {
        let pending = AsyncStream<CheckedContinuation<[BsvHomeAssetRefreshFailure], Never>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock(refreshHandler: {
            await withCheckedContinuation { pending.continuation.yield($0) }
        })
        let model = AssetSceneViewModel.mock(service: service)
        let sections = TransactionViewModel.sections([.mock()])
        model.transactionsQuery.value = sections
        model.transactionsState = .error(AnyError("Previous refresh failed"))
        let refresh = Task { await model.load() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()

        #expect(model.transactionsState.isLoading)
        #expect(model.transactionSections.flatMap(\.values) == sections.flatMap(\.values))

        completion?.resume(returning: [])
        await refresh.value
        #expect(!model.transactionsState.isLoading)
        #expect(!model.transactionsState.isError)
    }

    @Test
    func emptyHistoryIsSuccessfulOnlyAfterRefresh() async {
        let model = AssetSceneViewModel.mock()
        #expect(model.transactionsState.isLoading)

        await model.load()

        guard case .data = model.transactionsState else {
            Issue.record("Expected successful empty history")
            return
        }
        #expect(model.transactionSections.isEmpty)
    }

    @Test(arguments: [WalletType.view, .privateKey])
    func singleAddressWalletKeepsAddressWithoutDiscoveryControls(type: WalletType) async {
        let account = Account.mock(chain: .bitcoinSv, address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA")
        let service = BsvHomeAssetDetailsServiceMock()
        let model = AssetSceneViewModel.mock(
            .mock(asset: Chain.bitcoinSv.asset, account: account),
            wallet: .mock(type: type, accounts: [account]),
            service: service,
        )
        await model.prepareBsvAddresses()
        #expect(!model.showsBsvAddressDiscovery)
        #expect(!model.showsBsvRecovery)
        #expect(model.address == account.address)
        #expect(model.addressRecords.count == 1)
        model.onContinueBsvScan()
        #expect(!model.isScanningAddresses)
        model.onCopyAddress()
        #expect(model.copyModel.copyValue == account.address)
    }

    @Test
    func balanceFailureDoesNotMarkHistoryAsFailed() async {
        let service = BsvHomeAssetDetailsServiceMock(refreshFailures: [.init(step: .updateBalances, message: "Unavailable")])
        let model = AssetSceneViewModel.mock(service: service)

        await model.load()

        guard case .data = model.transactionsState else {
            Issue.record("Expected successful history independent of balance refresh")
            return
        }
        #expect(model.isPresentingToastMessage != nil)
    }

    @Test
    func swapPassesTheCurrentAssetToTheNavigationCallback() {
        let asset = Asset.mockTronUSDT()
        var selected: AssetId?
        var presented: SelectedAssetInput?
        let model = AssetSceneViewModel(
            service: BsvHomeAssetDetailsServiceMock(),
            preferences: .mock(),
            input: AssetSceneInput(wallet: .mock(), asset: asset),
            isPresentingSelectedAssetInput: Binding(get: { presented }, set: { presented = $0 }),
            onSwap: { selected = $0 },
        )

        model.onSelectHeader(.swap)

        #expect(selected == asset.id)
        #expect(presented == nil)
    }

    @Test
    func tokenDetailUsesOnlyTheKnownTronUsdtIdentity() {
        #expect(AssetSceneViewModel.mock(.mock(asset: .mockTronUSDT())).detailLayout == .token)
        #expect(AssetSceneViewModel.mock(.mock(asset: Chain.tronShasta.defaultAsset(type: .trc20))).detailLayout == .token)
        #expect(AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset)).detailLayout == .asset)
        #expect(AssetSceneViewModel.mock(.mock(asset: .mockEthereumUSDT())).detailLayout == .asset)
        let foreignUsdt = Asset(id: AssetId(chain: .tron, tokenId: "unrelated-contract"), name: "Tether USD", symbol: "USDT", decimals: 6, type: .trc20)
        #expect(AssetSceneViewModel.mock(.mock(asset: foreignUsdt)).detailLayout == .asset)
    }

    @Test
    func shastaActionsExcludePriceAlertsAndSwap() {
        for asset in [Chain.tronShasta.asset, Chain.tronShasta.defaultAsset(type: .trc20)] {
            let model = AssetSceneViewModel.mock(
                .mock(asset: asset),
                service: BsvHomeAssetDetailsServiceMock(headerActions: .buttons(buttons: [
                    BsvHomeHeaderButton(kind: .send, isEnabled: true),
                    BsvHomeHeaderButton(kind: .receive, isEnabled: true),
                    BsvHomeHeaderButton(kind: .swap, isEnabled: true),
                    BsvHomeHeaderButton(kind: .buy, isEnabled: true),
                ])),
            )

            #expect(!model.showsPriceAlerts)
            #expect(model.tokenButtons.map(\.type) == [.send, .receive])
            #expect(model.additionalButtons.isEmpty)
        }

        #expect(AssetSceneViewModel.mock(.mock(asset: .mockTronUSDT())).showsPriceAlerts)
    }

    @Test
    func tokenButtonsPreserveCoreAvailabilityInPrototypeOrder() {
        let model = AssetSceneViewModel.mock(
            .mock(asset: .mockTronUSDT()),
            service: BsvHomeAssetDetailsServiceMock(headerActions: .buttons(buttons: [
                BsvHomeHeaderButton(kind: .swap, isEnabled: true),
                BsvHomeHeaderButton(kind: .send, isEnabled: false),
                BsvHomeHeaderButton(kind: .buy, isEnabled: true),
                BsvHomeHeaderButton(kind: .receive, isEnabled: true),
            ])),
        )

        #expect(model.tokenButtons.map(\.type) == [.send, .receive, .swap])
        #expect(model.tokenButtons.map(\.isEnabled) == [false, true, true])
    }

    @Test
    func tokenButtonsDisableActionsMissingFromCore() {
        let model = AssetSceneViewModel.mock(
            .mock(asset: .mockTronUSDT()),
            service: BsvHomeAssetDetailsServiceMock(headerActions: .buttons(buttons: [
                BsvHomeHeaderButton(kind: .receive, isEnabled: true),
            ])),
        )

        #expect(model.tokenButtons.map(\.type) == [.send, .receive, .swap])
        #expect(model.tokenButtons.map(\.isEnabled) == [false, true, false])
    }

    @Test
    func tokenButtonsRemainDisabledForWatchWallets() {
        let model = AssetSceneViewModel.mock(
            .mock(asset: .mockTronUSDT()),
            wallet: .mock(type: .view),
            service: BsvHomeAssetDetailsServiceMock(headerActions: .buttons(buttons: [
                BsvHomeHeaderButton(kind: .send, isEnabled: true),
                BsvHomeHeaderButton(kind: .receive, isEnabled: true),
                BsvHomeHeaderButton(kind: .swap, isEnabled: true),
            ])),
        )

        #expect(model.tokenButtons.map(\.type) == [.send, .receive, .swap])
        #expect(model.tokenButtons.allSatisfy { !$0.isEnabled })
        #expect(!model.isHeaderActionEnabled(.receive))
    }

    @Test(arguments: [Chain.tron.asset, .mockTronUSDT()], [true, false, nil] as [Bool?])
    func receiveAvailabilityFollowsCoreWithAnExistingAddress(asset: Asset, isEnabled: Bool?) {
        let model = AssetSceneViewModel.mock(
            .mock(asset: asset, account: .mock(chain: .tron, address: "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH")),
            service: BsvHomeAssetDetailsServiceMock(headerActions: .buttons(buttons: isEnabled.map { [BsvHomeHeaderButton(kind: .receive, isEnabled: $0)] } ?? [])),
        )

        #expect(model.address.isNotEmpty)
        #expect(model.isHeaderActionEnabled(.receive) == (isEnabled == true))
    }

    @Test
    func assetBalancesUseDisplayPrecisionAndMissingPrice() {
        let bsv = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset, balance: .mock(available: BigInt(123_456_789))))
        #expect(bsv.assetHeaderModel.amountTitle == "1.23456789")
        #expect(bsv.assetHeaderModel.amountSymbol == "BSV")
        #expect(bsv.assetHeaderModel.subtitle == "—")

        let usdt = AssetSceneViewModel.mock(.mock(asset: .mockTronUSDT(), balance: .mock(available: BigInt(1_234_567)), price: .mock(price: 1)))
        #expect(usdt.assetHeaderModel.amountTitle == "1.2345")
        #expect(usdt.assetKind == "TRON (TRC20)")
        #expect(usdt.assetHeaderModel.subtitle == "$1.234567")
        let zero = AssetSceneViewModel.mock(.mock(asset: .mockTronUSDT(), balance: .zero, price: .mock(price: 1)))
        #expect(zero.assetHeaderModel.subtitle == "$0.0000")
    }

    @Test(arguments: [true, false])
    func priceChangeVisibilityFollowsCoreWithoutHidingValuation(showsPriceChange: Bool) {
        let model = AssetSceneViewModel.mock(
            .mock(asset: .mockTronUSDT(), balance: .mock(available: BigInt(1_234_567)), price: .mock(price: 1, priceChangePercentage24h: 2)),
            service: BsvHomeAssetDetailsServiceMock(showsPriceChange: showsPriceChange),
        )

        #expect(model.isPriceChangeAvailable == showsPriceChange)
        #expect(model.assetHeaderModel.subtitle == "$1.234567")
    }

    @Test
    func missingPriceHidesPriceChangeEvenWhenCoreAllowsIt() {
        let model = AssetSceneViewModel.mock(
            .mock(asset: .mockEthereum()),
            service: BsvHomeAssetDetailsServiceMock(showsPriceChange: true),
        )

        #expect(!model.isPriceChangeAvailable)
        #expect(model.assetHeaderModel.subtitle == "—")
    }

    @Test
    func missingAddressCannotBeCopied() {
        let model = AssetSceneViewModel.mock(.mock(asset: .mockTronUSDT(), account: .mock(chain: .tron)))
        model.onCopyAddress()
        #expect(!model.isPresentingCopyToast)
    }

    @Test
    func bsvAddressListExcludesChangeAndUnallocatedReserveAddresses() async {
        let receivingRecords = (0 ..< 12).map { (index: Int) -> BsvHomeBsvAddressRecord in
            let address = index == 0 ? "identity-address" : "receive-\(index)"
            let status: BsvHomeBsvAddressStatus = index == 0 ? .used : .unused
            return BsvHomeBsvAddressRecord(branch: .receive, index: UInt32(index), address: address, status: status, isCurrent: index == 1, isMain: index == 0)
        }
        let records = receivingRecords + [BsvHomeBsvAddressRecord(branch: .change, index: 0, address: "change-0", status: .used, isCurrent: false, isMain: false)]
        let service = BsvHomeAssetDetailsServiceMock()
        service.bsvSnapshot = .mock(records: records, currentAddress: "receive-1", receiveCount: 10)
        let model = AssetSceneViewModel.mock(
            .mock(asset: Chain.bitcoinSv.asset, account: .mock(chain: .bitcoinSv, address: "identity-address")),
            service: service,
        )

        await model.prepareBsvAddresses()
        #expect(model.addressRecords == Array(receivingRecords.prefix(10)))
        #expect(model.address == "receive-1")
        #expect(model.assetData.account.address == "identity-address")
        model.onCopyAddress("receive-2")
        #expect(model.copyModel.copyValue == "receive-2")
        model.isPresentingCopyToast = false
        model.onCopyAddress()
        #expect(model.copyModel.copyValue == "receive-1")
    }

    @Test
    func addingAnAddressUpdatesCopyAndExplorerWithoutChangingIdentity() async throws {
        let initial = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let next = "1cMh228HTCiwS8ZsaakH8A8wze1JR5ZsP"
        let service = BsvHomeAssetDetailsServiceMock()
        service.addBsvHandler = { _ in .mock(records: [
            BsvHomeBsvAddressRecord(branch: .receive, index: 0, address: initial, status: .used, isCurrent: false, isMain: true),
            BsvHomeBsvAddressRecord(branch: .receive, index: 1, address: next, status: .unused, isCurrent: true, isMain: false),
        ], currentAddress: next) }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset, account: .mock(chain: .bitcoinSv, address: initial)), service: service)
        await model.prepareBsvAddresses()

        await model.onAddBsvAddress()?.value

        #expect(service.addedBsvWalletIds == [model.input.wallet.id.id])
        #expect(model.address == next)
        #expect(model.assetData.account.address == initial)
        model.onCopyAddress()
        #expect(model.copyModel.copyValue == next)
        #expect(!model.addressesState.isError)
        guard case let .button(_, _, _, action) = try #require(model.menuItems.first) else {
            Issue.record("Expected a receiving-address explorer action")
            return
        }
        let openExplorer = try #require(action)
        openExplorer()
        guard case let .url(url)? = model.isPresentingAssetSheet else {
            Issue.record("Expected the current receiving address explorer")
            return
        }
        #expect(url.absoluteString.hasSuffix(next))
    }

    @Test
    func leavingDuringAddressSelectionDiscardsTheOldWalletResponse() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, Never>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock()
        service.selectBsvHandler = { _, _ in
            await withCheckedContinuation { pending.continuation.yield($0) }
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()
        let previousWalletId = model.input.wallet.id
        let selection = model.onSelectBsvAddress(index: 1)
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        #expect(model.isUpdatingAddress)
        #expect(model.onAddBsvAddress() == nil)

        model.input = AssetSceneInput(wallet: .mock(id: .multicoin(address: "replacement-wallet")), asset: Chain.bitcoinSv.asset)
        completion?.resume(returning: .mock(currentAddress: "late-public-address"))
        await selection?.value

        #expect(service.selectedBsvAddresses.map(\.walletId) == [previousWalletId.id])
        #expect(service.selectedBsvAddresses.map(\.index) == [1])
        #expect(service.addedBsvWalletIds.isEmpty)
        #expect(model.address.isEmpty)
        #expect(model.bsvAddressSnapshot == nil)
        #expect(!model.isUpdatingAddress)
    }

    @Test
    func unavailableAddressAllocationDoesNotCallTheService() async {
        let service = BsvHomeAssetDetailsServiceMock()
        service.bsvSnapshot = .mock(canAdd: false)
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()

        #expect(model.onAddBsvAddress() == nil)
        #expect(service.addedBsvWalletIds.isEmpty)
    }

    @Test
    func bsvPreparationCancellationCannotExposeTheIdentityAsReceivingAddress() async {
        let service = BsvHomeAssetDetailsServiceMock()
        service.prepareBsvHandler = { throw CancellationError() }
        let model = AssetSceneViewModel.mock(
            .mock(asset: Chain.bitcoinSv.asset, account: .mock(chain: .bitcoinSv, address: "identity-address")),
            service: service,
        )
        await model.prepareBsvAddresses()
        #expect(model.address.isEmpty)
        #expect(model.addressesState.isError)
        model.onCopyAddress()
        #expect(!model.isPresentingCopyToast)

        service.prepareBsvHandler = nil
        await model.prepareBsvAddresses()
        #expect(model.address == service.bsvSnapshot.currentAddress)
        #expect(!model.addressesState.isError)
        model.onCopyAddress()
        #expect(model.copyModel.copyValue == service.bsvSnapshot.currentAddress)
    }

    @Test
    func bsvLifecycleRefreshUpdatesAddressAndCopyWithoutChangingIdentity() async {
        let initialAddress = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let nextAddress = "1cMh228HTCiwS8ZsaakH8A8wze1JR5ZsP"
        let service = BsvHomeAssetDetailsServiceMock()
        let model = AssetSceneViewModel.mock(
            .mock(asset: Chain.bitcoinSv.asset, account: .mock(chain: .bitcoinSv, address: initialAddress)),
            service: service,
        )
        await model.prepareBsvAddresses()
        service.bsvSnapshot = .mock(records: [
            BsvHomeBsvAddressRecord(branch: .receive, index: 0, address: initialAddress, status: .used, isCurrent: false, isMain: true),
            BsvHomeBsvAddressRecord(branch: .receive, index: 1, address: nextAddress, status: .unused, isCurrent: true, isMain: false),
        ], currentAddress: nextAddress)

        await model.load()
        #expect(model.address == nextAddress)
        #expect(model.assetData.account.address == initialAddress)
        model.onCopyAddress()
        #expect(model.copyModel.copyValue == nextAddress)
        #expect(!model.addressesState.isError)
    }

    @Test
    func completedHdDiscoveryKeepsContinueRecoveryAvailable() async throws {
        let address = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
        let account = Account.mock(chain: .bitcoinSv, address: address)
        let service = BsvHomeAssetDetailsServiceMock()
        service.bsvSnapshot = .mock(currentAddress: address, receiveScanned: 20, changeScanned: 20, selectedIndex: 0, discovery: .complete)
        let continued = Locked(wrappedValue: false)
        service.continueBsvHandler = {
            continued.wrappedValue = true
            return .mock(currentAddress: address, receiveScanned: 40, changeScanned: 40, selectedIndex: 0, discovery: .complete)
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset, account: account), wallet: .mock(accounts: [account]), service: service)
        await model.prepareBsvAddresses()

        #expect(model.showsBsvRecovery)
        model.onContinueBsvScan()
        try await waitFor { continued.wrappedValue && !model.isScanningAddresses }

        #expect(model.bsvAddressSnapshot?.receiveScanned == 40)
        #expect(model.bsvAddressSnapshot?.selectedIndex == 0)
        #expect(model.showsBsvRecovery)
        #expect(model.address == address)
        #expect(model.assetData.account.address == address)
        #expect(!model.addressesState.isError)
    }

    @Test
    func pendingPreparationCannotBePreemptedByContinueRecovery() async throws {
        let service = BsvHomeAssetDetailsServiceMock()
        let snapshot = BsvHomeBsvAddressSnapshot.mock(receiveScanned: 20, changeScanned: 20, selectedIndex: 0, discovery: .complete)
        service.bsvSnapshot = snapshot
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        service.prepareBsvHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let continued = Locked(wrappedValue: false)
        service.continueBsvHandler = {
            continued.wrappedValue = true
            return snapshot
        }
        let retry = Task { await model.prepareBsvAddresses() }
        var preparations = pending.stream.makeAsyncIterator()
        let completion = await preparations.next()

        model.onContinueBsvScan()

        #expect(model.showsBsvRecovery)
        #expect(model.addressesState.isLoading)
        #expect(!model.isScanningAddresses)
        #expect(!continued.wrappedValue)
        completion?.resume(returning: snapshot)
        await retry.value
        #expect(!model.addressesState.isError)
        model.onContinueBsvScan()
        try await waitFor { continued.wrappedValue && !model.isScanningAddresses }
    }

    @Test
    func bsvScanShowsPersistedProgressAndCanRetryAfterCancellation() async throws {
        let service = BsvHomeAssetDetailsServiceMock()
        service.continueBsvHandler = {
            try await Task.sleep(for: .seconds(60))
            return .mock()
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()
        model.onContinueBsvScan()
        service.bsvSnapshot = .mock(receiveScanned: 25, changeScanned: 20)

        try await waitFor { model.bsvAddressSnapshot?.receiveScanned == 25 }
        #expect(model.isScanningAddresses)
        model.onCancelBsvScan()
        try await waitFor { !model.isScanningAddresses }
        #expect(model.addressesState.isError)
        #expect(model.bsvAddressSnapshot?.receiveScanned == 25)

        service.continueBsvHandler = nil
        model.onContinueBsvScan()
        try await waitFor { !model.isScanningAddresses }
        #expect(!model.addressesState.isError)
        #expect(model.bsvAddressSnapshot?.changeScanned == 20)
    }

    @Test
    func bsvPreparationFromPreviousWalletCannotPublishAnAddress() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock()
        service.prepareBsvHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        let preparation = Task { await model.prepareBsvAddresses() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        model.input = AssetSceneInput(wallet: .mock(id: .multicoin(address: "different-wallet")), asset: Chain.bitcoinSv.asset)
        completion?.resume(returning: .mock())
        await preparation.value

        #expect(model.address.isEmpty)
        #expect(model.addressesState.isNoData)
        #expect(model.copyModel.copyValue.isEmpty)
        #expect(model.bsvAddressSnapshot == nil)
    }

    @Test
    func bsvPreparationRetryShowsLoadingAndDoesNotRepeatAuthentication() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock()
        service.prepareBsvHandler = { throw CancellationError() }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()
        #expect(model.addressesState.isError)

        service.prepareBsvHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let preparation = Task { await model.prepareBsvAddresses() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        #expect(model.addressesState.isLoading)

        service.prepareBsvHandler = { throw AnyError("Repeated authentication") }
        await model.prepareBsvAddresses()
        #expect(model.addressesState.isLoading)

        completion?.resume(returning: .mock(currentAddress: nil, allocationPaused: true))
        await preparation.value
        #expect(!model.addressesState.isError)
    }

    @Test
    func cancelledPreparationReturningNormallyCanBeRetried() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock()
        service.prepareBsvHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        let preparation = Task { await model.prepareBsvAddresses() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        preparation.cancel()
        completion?.resume(returning: .mock())
        await preparation.value
        #expect(model.addressesState.isError)
        #expect(!model.addressesState.isLoading)
        #expect(model.address.isEmpty)

        service.prepareBsvHandler = nil
        await model.prepareBsvAddresses()
        #expect(!model.addressesState.isError)
        #expect(model.address.isNotEmpty)
    }

    @Test(arguments: [false, true])
    func cancellingRefreshDoesNotRestartScanningThroughBalanceRefresh(continueScan: Bool) async throws {
        let service = BsvHomeAssetDetailsServiceMock()
        service.syncBsvHandler = {
            try await Task.sleep(for: .seconds(60))
            return .mock()
        }
        service.continueBsvHandler = service.syncBsvHandler
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        await model.prepareBsvAddresses()
        if continueScan {
            model.onContinueBsvScan()
        }
        model.transactionsState = .data(())
        let refresh = Task { await model.load() }
        try await waitFor { model.isScanningAddresses && model.transactionsState.isLoading }
        model.onCancelBsvScan()
        await refresh.value
        #expect(service.refreshCalls == 0)
        #expect(service.cancelledBsvWalletIds == [model.input.wallet.id.id])
        #expect(model.addressesState.isError)
        #expect(!model.transactionsState.isLoading)
    }

    @Test
    func leavingDuringInitialPreparationCannotStartScanningAfterReturn() async throws {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeAssetDetailsServiceMock()
        service.prepareBsvHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = AssetSceneViewModel.mock(.mock(asset: Chain.bitcoinSv.asset), service: service)
        model.loadOnce()
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        model.onCancelBsvScan()
        completion?.resume(returning: .mock())
        try await Task.sleep(for: .milliseconds(20))
        #expect(model.address.isEmpty)
        #expect(!model.isScanningAddresses)
        #expect(service.refreshCalls == 0)
    }

    private func waitFor(_ condition: @MainActor () -> Bool) async throws {
        for _ in 0 ..< 200 {
            if condition() {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        Issue.record("Timed out waiting for BSV address state")
    }

    @Test
    func swapAssetTypeUsesTheAssetWhenCoreSuggestsNoReceiveAsset() {
        let asset = Asset.mockEthereumUSDT()
        let model = AssetSceneViewModel.mock(
            .mock(asset: asset, balance: .mock()),
            service: BsvHomeAssetDetailsServiceMock(assetPair: BsvHomeSwapPairSuggestion(payAssetId: asset.id.identifier, receiveAssetId: nil)),
        )

        #expect(model.swapAssetType == .swap(asset, nil))
    }

    @Test
    func swapAssetTypePaysWithTheChainAssetWhenCoreSuggestsAReceiveAsset() {
        let asset = Asset.mockEthereumUSDT()
        let model = AssetSceneViewModel.mock(
            .mock(asset: asset, balance: .zero),
            service: BsvHomeAssetDetailsServiceMock(
                assetPair: BsvHomeSwapPairSuggestion(payAssetId: asset.chain.assetId.identifier, receiveAssetId: asset.id.identifier),
            ),
        )

        #expect(model.swapAssetType == .swap(asset.chain.asset, asset))
    }

    @Test
    func balanceRows() {
        let ethereum = AssetSceneViewModel.mock(
            .mock(
                asset: .mockEthereum(),
                balance: .mock(staked: BigInt(6_000_000_000_000_000_000), earn: BigInt(4_000_000_000_000_000_000)),
            ),
        )
        let rows = ethereum.balanceRows
        #expect(rows.count == 3)
        guard case let .staked(staked) = rows[1], case let .earn(earn) = rows[2] else {
            Issue.record("Expected available, staked and earn rows")
            return
        }
        #expect(ethereum.stakeBalanceText(staked) == "6.0000 ETH")
        #expect(ethereum.balanceText(earn) == "4.0000 ETH")
        #expect(AssetSceneViewModel.mock(.mock(asset: .mockEthereum(), metadata: .mock(isStakeEnabled: false))).balanceRows.isEmpty)
        #expect(AssetSceneViewModel.mock(.mock(asset: .mockEthereum(), metadata: .mock(isStakeEnabled: true))).balanceRows.isEmpty)
    }

    @Test
    func disabledStakingHidesPromotionsAndRejectsNavigation() {
        let asset = Asset.mockEthereum()
        var presented: SelectedAssetInput?
        let model = AssetSceneViewModel(
            service: BsvHomeAssetDetailsServiceMock(),
            preferences: .mock(),
            input: AssetSceneInput(wallet: .mock(), asset: asset),
            isPresentingSelectedAssetInput: Binding(get: { presented }, set: { presented = $0 }),
        )
        model.assetQuery.value = ChainAssetData(
            assetData: .mock(asset: asset, balance: .zero, metadata: .mock(isStakeEnabled: true)),
            feeAssetData: .with(asset: asset),
        )
        model.bannersQuery.value = [
            .mock(walletId: nil, asset: asset, event: .stake, state: .active),
            .mock(walletId: nil, asset: asset, event: .accountBlockedMultiSignature, state: .alwaysActive),
        ]

        #expect(model.balanceRows.isEmpty)
        #expect(model.visibleBanners.map(\.event) == [.accountBlockedMultiSignature])
        model.onSelectHeader(.stake)
        #expect(presented == nil)
        model.onSelectHeader(.receive)
        #expect(presented?.type == .receive(.asset))
    }

    @Test
    func balanceTitle() {
        let model = AssetSceneViewModel.mock()
        #expect(model.balanceTitle(for: .stake).isEmpty == false)
        #expect(model.balanceTitle(for: .earn).isEmpty == false)
    }
}

// MARK: - Mock Extensions

extension AssetSceneViewModel {
    static func mock(
        _ assetData: AssetData = AssetData.mock(),
        wallet: Wallet = .mock(),
        service: any BsvHomeAssetDetailsServiceProtocol = BsvHomeAssetDetailsServiceMock(),
    ) -> AssetSceneViewModel {
        let model = AssetSceneViewModel(
            service: service,
            preferences: .mock(),
            input: AssetSceneInput(
                wallet: wallet,
                asset: assetData.asset,
            ),
            isPresentingSelectedAssetInput: .constant(.none),
        )
        model.assetQuery.value = ChainAssetData(
            assetData: assetData,
            feeAssetData: AssetData.with(asset: assetData.asset.chain.asset),
        )
        return model
    }
}
