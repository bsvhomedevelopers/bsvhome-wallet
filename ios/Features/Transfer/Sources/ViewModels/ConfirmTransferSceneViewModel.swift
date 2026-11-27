// Copyright (c). Gem Wallet. All rights reserved.

import BigInt
import Components
import Formatters
import Foundation
import struct BsvHomeCore.BsvHomeConfirmButton
import enum BsvHomeCore.BsvHomeConfirmError
import enum BsvHomeCore.BsvHomeConfirmFeeRow
import enum BsvHomeCore.BsvHomeConfirmFeeSelection
import struct BsvHomeCore.BsvHomeConfirmLoad
import struct BsvHomeCore.BsvHomeConfirmLoadOptions
import protocol BsvHomeCore.BsvHomeConfirmSessionProtocol
import struct BsvHomeCore.BsvHomeConfirmSimulationState
import enum BsvHomeCore.BsvHomeExecuteResult
import struct BsvHomeCore.BsvHomeFeeRateRows
import protocol BsvHomeCore.BsvHomePreferencesServiceProtocol
import struct BsvHomeCore.BsvHomeSimulationWarningRow
import enum BsvHomeCore.BsvHomeTransferAmountResult
import struct BsvHomeCore.BsvHomeTransferData
import struct BsvHomeCore.SimulationPayloadField
import struct BsvHomeCore.SimulationResult
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import Store
import Swap
import SwiftUI
import WalletConnector

@Observable
@MainActor
public final class ConfirmTransferSceneViewModel: Identifiable {
    public let id = UUID()
    private enum LoadErrorPresentation: Equatable {
        case automatic
        case inlineBalanceErrors
    }

    var feeSelection: BsvHomeConfirmFeeSelection {
        didSet { feeRates = state.feeRateRows(selection: feeSelection) }
    }

    var feeAssetSelection: FeeAssetSelection
    var state: ConfirmTransferState {
        didSet { onStateChange(state: state) }
    }

    private(set) var button: BsvHomeConfirmButton
    private(set) var feeRow: BsvHomeConfirmFeeRow
    private(set) var feeRates: BsvHomeFeeRateRows?

    public var isPresentingSheet: ConfirmTransferSheetType?

    public var isPresentingAlertMessage: AlertMessage?

    private let request: ConfirmTransferRequest
    private let additionalDetails: [ListItemModel]
    private let wallet: Wallet
    private let onComplete: VoidAction
    private let context: ConfirmTransferContext

    private var loadRevision = 0
    private var loadTask: Task<BsvHomeConfirmLoad, any Error>?
    private(set) var executionTask: Task<Void, Never>?
    private var executionRevision: UInt = 0
    private var loadErrorPresentation: LoadErrorPresentation = .automatic

    private let session: any BsvHomeConfirmSessionProtocol
    public let passwordAuthorization: TransferPasswordAuthorization?
    public private(set) var bsvReceipt: BsvSendReceipt?

    public init(
        request: ConfirmTransferRequest,
        wallet: Wallet,
        session: any BsvHomeConfirmSessionProtocol,
        additionalDetails: [ListItemModel] = [],
        context: ConfirmTransferContext = .standard,
        passwordAuthorization: TransferPasswordAuthorization? = nil,
        onComplete: VoidAction,
    ) {
        self.request = request
        self.wallet = wallet
        self.session = session
        self.additionalDetails = additionalDetails
        self.context = context
        self.passwordAuthorization = passwordAuthorization
        self.onComplete = onComplete

        let feeSelection = BsvHomeConfirmFeeSelection.priority(priority: request.data.defaultFeePriority())
        let state = ConfirmTransferState(
            transfer: request.data,
            simulation: ConfirmSimulationState(result: request.simulation, chain: request.data.chain),
            screen: session.screen(),
        )
        let screen = state.screen
        self.feeSelection = feeSelection
        self.state = state
        feeAssetSelection = .automatic
        button = screen.button()
        feeRow = screen.feeRow()
        feeRates = state.feeRateRows(selection: feeSelection)
    }

    var preloadSelection: ConfirmPreloadSelection {
        ConfirmPreloadSelection(fee: feeSelection, feeAsset: feeAssetSelection)
    }

    private var errorInfoAction: ((ConfirmTransferError) -> Void)? {
        guard context != .trxAcquisition || state.transactionError?.isInsufficientBalance != true else { return nil }
        return onSelectListError
    }

    var title: String {
        dataModel.title
    }

    var websiteURL: URL? {
        dataModel.websiteURL
    }

    var websiteTitle: String {
        Localized.Settings.website
    }

    var senderExplorerContext: ExplorerContextData {
        ExplorerContextData(
            copyValue: .address(value: senderAddress, chain: dataModel.chain),
            explorerLink: explorerLink(chain: dataModel.chain, address: senderAddress),
        )
    }

    var progressMessage: String {
        Localized.Common.loading
    }

    public var isConfirming: Bool {
        state.screen.phase == .confirming
    }

    var isHeaderVisible: Bool {
        guard request.data.applicationMetadata?.source == .payment else {
            return true
        }
        return state.preload != nil
    }

    var simulationWarnings: [BsvHomeSimulationWarningRow] {
        state.simulation.warnings
    }

    public var payloadModel: SimulationPayloadModel { state.simulation.payload }

    var confirmButtonModel: ConfirmButtonViewModel {
        ConfirmButtonViewModel(
            button: button,
            authentication: session.authentication(),
            onAction: { [weak self] in self?.onSelectConfirm() },
        )
    }

    public var detailsViewModel: ConfirmDetailsViewModel {
        ConfirmDetailsViewModel(
            type: request.data.inputType,
            metadata: state.metadata,
            session: session,
        )
    }

    var balanceChangeModels: [ConfirmBalanceChangeViewModel] {
        state.simulation.balanceChanges.map(ConfirmBalanceChangeViewModel.init)
    }

    public var feeModel: NetworkFeeSceneViewModel {
        NetworkFeeSceneViewModel(
            feeAsset: state.feeAsset,
            currency: session.currency,
            selection: feeSelection,
            feeRates: feeRates,
            feeAssetPrice: state.metadata?.feePrice,
            feeAmount: state.fee?.fee,
            feeAssets: state.feeAssets.map { $0.feeAssetItem(currency: session.currency) },
            onSelect: { [weak self] in self?.feeSelection = $0 },
            onSelectFeeAsset: { [weak self] in self?.selectFeeAsset($0) },
        )
    }
}

// MARK: - ListSectionProvideable

extension ConfirmTransferSceneViewModel: ListSectionProvideable {
    public var sections: [ListSection<ConfirmTransferItem>] {
        [
            ListSection(type: .header, [.header]),
            ListSection(type: .details, detailItems),
            additionalDetails.isEmpty ? nil : ListSection(type: .additionalDetails, additionalDetails.indices.map(ConfirmTransferItem.additionalDetail)),
            simulationWarnings.isEmpty ? nil : ListSection(type: .warnings, [.warnings]),
            payloadModel.primaryFields.isEmpty ? nil : ListSection(type: .payload, [.payload]),
            balanceChangeModels.isEmpty ? nil : ListSection(type: .balanceChanges, balanceChangeModels.indices.map(ConfirmTransferItem.balanceChange)),
            ListSection(type: .fee, [.networkFee]),
            ListSection(type: .error, [.error]),
        ].compactMap(\.self)
    }

    private var detailItems: [ConfirmTransferItem] {
        if case .generic = request.data.inputType {
            return [.app, .sender, .network]
        }
        return [.app, .sender, .recipient, .network, .memo, .details]
    }

    public func itemModel(for item: ConfirmTransferItem) -> any ItemModelProvidable<ConfirmTransferItemModel> {
        switch item {
        case .header:
            ConfirmHeaderViewModel(request: request, state: state, currency: session.currency)
        case .warnings:
            ConfirmTransferItemModel.warnings(simulationWarnings)
        case .app:
            ConfirmAppViewModel(transfer: request.data)
        case .sender:
            ConfirmSenderViewModel(wallet: wallet)
        case .network:
            ConfirmNetworkViewModel(transfer: request.data)
        case .recipient:
            ConfirmRecipientViewModel(
                destination: request.data.destination()?.withAddressName(addressName: state.addressName?.map()),
                chain: dataModel.chain,
                memo: dataModel.recipient.memo,
                addressName: state.addressName,
                addressLink: explorerLink(chain: dataModel.chain, address: dataModel.recipient.address),
            )
        case .memo:
            ConfirmMemoViewModel(transfer: request.data)
        case .details:
            detailsViewModel
        case .payload:
            ConfirmTransferItemModel.payload(payloadModel.primaryFields)
        case let .additionalDetail(index):
            ConfirmTransferItemModel.additionalDetail(additionalDetails[index])
        case let .balanceChange(index):
            ConfirmTransferItemModel.balanceChange(balanceChangeModels[index])
        case .networkFee:
            ConfirmNetworkFeeViewModel(
                feeRow: feeRow,
                feeModel: feeModel,
                infoAction: onSelectNetworkFeeInfo,
            )
        case .error:
            ConfirmErrorViewModel(
                error: state.transactionError,
                onSelectListError: errorInfoAction,
            )
        }
    }
}

// MARK: - Business Logic

extension ConfirmTransferSceneViewModel {
    func onSelectListError(error: ConfirmTransferError) {
        let isInsufficientBalance = error.isInsufficientBalance
        guard context != .trxAcquisition || !isInsufficientBalance else { return }
        guard let sheet = ConfirmInfoSheetBuilder.build(
            for: error,
            feePrice: state.metadata?.feePrice,
            prices: state.metadata?.assetPrices ?? [:],
            currency: session.currency.rawValue,
            acquireFlow: { session.acquireAssetFlow(chain: $0.chain.rawValue) },
            networkFeeBuyAmount: Int(session.insufficientNetworkFeeBuyAmount()),
            onGetAsset: { [weak self] asset, buyAmount in
                if isInsufficientBalance, asset.id == Chain.tron.asset.id {
                    self?.isPresentingSheet = .trxExchange
                } else {
                    self?.onSelectGetAsset(asset, buyAmount: buyAmount)
                }
            },
        ) else { return }
        isPresentingSheet = .info(sheet)
    }

    func onSelectNetworkFeeInfo() {
        isPresentingSheet = .info(.networkFee(state.feeAsset))
    }

    public func contextMenuItems(for field: SimulationPayloadField) -> [ContextMenuItemType] {
        payloadModel.contextMenuItems(
            for: field,
            explorerLink: { explorerLink(chain: dataModel.chain, address: $0) },
            onOpenURL: { [weak self] in self?.isPresentingSheet = .url($0) },
        )
    }

    func onSelectPayloadDetails() {
        isPresentingSheet = .payloadDetails
    }

    func onSelectOpenWebsiteURL() {
        if let websiteURL {
            isPresentingSheet = .url(websiteURL)
        }
    }

    func onSelectFeePicker() {
        guard !isConfirming else { return }
        isPresentingSheet = .networkFeeSelector
    }

    func onSelectSwapDetails() {
        isPresentingSheet = .swapDetails
    }

    func onSelectPerpetualDetails(_ model: PerpetualDetailsViewModel) {
        isPresentingSheet = .perpetualDetails(model)
    }

    func onSelectConfirm() {
        switch state.screen.action() {
        case .load: Task { await load() }
        case .execute: confirm()
        case .none: break
        }
    }

    func load() async {
        await load(errorPresentation: .automatic)
    }

    public func refreshAfterTrxExchange() async {
        await load(errorPresentation: .inlineBalanceErrors)
    }

    private func load(errorPresentation: LoadErrorPresentation) async {
        guard !isConfirming else { return }
        loadRevision += 1
        let revision = loadRevision
        let previousLoad = loadTask
        loadErrorPresentation = errorPresentation
        defer {
            if revision == loadRevision {
                loadTask = nil
                loadErrorPresentation = .automatic
            }
        }
        state.screen = state.screen.onLoadStarted()
        let task = Task { [weak self] in
            _ = await previousLoad?.result
            try Task.checkCancellation()
            guard let self, revision == loadRevision else { throw CancellationError() }
            let initialLoad = try await session.state()
            try Task.checkCancellation()
            guard revision == loadRevision else { throw CancellationError() }
            state = try ConfirmTransferState(initialLoad, screen: state.screen)
            return try await session.load(options: options(selection: feeSelection, feeAssetSelection: feeAssetSelection))
        }
        loadTask = task
        do {
            let load = try await withTaskCancellationHandler {
                try await task.value
            } onCancel: {
                task.cancel()
            }
            guard revision == loadRevision, !Task.isCancelled else { return }
            state = try ConfirmTransferState(load, screen: state.screen.onLoaded(load: load))
        } catch {
            guard revision == loadRevision, !Task.isCancelled else { return }
            state.screen = state.screen.onLoadFailed(error: error.confirmError)
            debugLog("confirm load error: \(error)")
        }
    }

    private func onStateChange(state: ConfirmTransferState) {
        let screen = state.screen
        button = screen.button()
        feeRow = screen.feeRow()
        feeRates = state.feeRateRows(selection: feeSelection)
        guard let error = state.transactionError else { return }
        guard loadErrorPresentation != .inlineBalanceErrors || !error.isInsufficientBalance else { return }
        switch error {
        case .confirm:
            onSelectListError(error: error)
        case .other:
            break
        }
    }

    private func selectFeeAsset(_ assetId: AssetId) {
        guard state.feeAsset.id != assetId else { return }
        feeAssetSelection = .selected(assetId)
    }
}

// MARK: - Private

public extension ConfirmTransferSceneViewModel {
    private func onSelectGetAsset(_ asset: Asset, buyAmount: Int? = nil) {
        switch session.acquireAssetFlow(chain: asset.chain.rawValue) {
        case .options:
            isPresentingSheet = .getAsset(asset, buyAmount: buyAmount)
        case .fiat:
            isPresentingSheet = .fiatConnect(
                assetAddress: AssetAddress(asset: asset, address: senderAddress),
                wallet: wallet,
                amount: buyAmount,
            )
        case .receive:
            isPresentingSheet = .receive(assetAddress: assetAddress(asset), wallet: wallet)
        }
    }

    private func confirm() {
        guard executionTask == nil else { return }
        executionRevision &+= 1
        let revision = executionRevision
        state.screen = state.screen.onExecuteStarted()
        executionTask = Task { [weak self] in
            guard let self else { return }
            defer {
                if revision == executionRevision {
                    executionTask = nil
                }
            }
            do {
                try await submit(request: request)
                guard revision == executionRevision, !Task.isCancelled else { return }
                if bsvReceipt == nil {
                    onComplete?()
                }
            } catch BsvHomeConfirmError.Cancelled {
                guard revision == executionRevision else { return }
                state.screen = state.screen.onExecuteCancelled()
            } catch {
                guard revision == executionRevision, !Task.isCancelled else { return }
                if context == .trxAcquisition, ConfirmTransferError(error: error).isInsufficientBalance {
                    state.screen = state.screen.onLoadFailed(error: error.confirmError)
                } else {
                    state.screen = state.screen.onExecuteFailed(error: error.confirmError)
                    isPresentingAlertMessage = AlertMessage(title: Localized.Errors.transferError, message: error.localizedDescription)
                }
                debugLog("confirm transaction error: \(error)")
            }
        }
    }

    internal func makeExecutionLifetime() -> ConfirmExecutionLifetime {
        ConfirmExecutionLifetime(session: session, model: self)
    }

    func cancelExecution() {
        executionRevision &+= 1
        session.cancel()
        passwordAuthorization?.cancel()
        executionTask?.cancel()
        executionTask = nil
        if isConfirming {
            state.screen = state.screen.onExecuteCancelled()
        }
    }

    func completeBsvReceipt() {
        guard bsvReceipt != nil else { return }
        onComplete?()
    }

    private var senderAddress: String {
        state.load?.sender.address ?? ""
    }

    func assetAddress(_ asset: Asset) -> AssetAddress {
        AssetAddress(asset: asset, address: senderAddress)
    }

    func swapFromAsset(to asset: Asset) -> Asset {
        dataModel.asset.id == asset.id ? state.feeAsset : dataModel.asset
    }

    var assetAcquisitionWallet: Wallet {
        wallet
    }

    private var dataModel: TransferDataViewModel {
        TransferDataViewModel(data: request.data)
    }
}

// MARK: - Confirm

extension ConfirmTransferSceneViewModel {
    func explorerLink(chain: Chain, address: String) -> BlockExplorerLink {
        session.explorerLink(chain: chain, address: address)
    }

    private func options(selection: BsvHomeConfirmFeeSelection, feeAssetSelection: FeeAssetSelection) -> BsvHomeConfirmLoadOptions {
        BsvHomeConfirmLoadOptions(
            feeSelection: selection,
            feeAssetId: feeAssetSelection.selectedAssetId?.identifier,
        )
    }

    func submit(request: ConfirmTransferRequest) async throws {
        let revision = executionRevision
        let result: BsvHomeExecuteResult
        do {
            result = try await session.execute()
        } catch let BsvHomeConfirmError.Broadcast(hashes, msg) {
            guard revision == executionRevision, !Task.isCancelled else { throw BsvHomeConfirmError.Cancelled }
            hashes.forEach { request.delegate?(.success($0)) }
            throw BsvHomeConfirmError.Broadcast(hashes: hashes, msg: msg)
        }
        guard revision == executionRevision, !Task.isCancelled else { throw BsvHomeConfirmError.Cancelled }
        switch result {
        case let .signed(data):
            data.forEach { request.delegate?(.success($0)) }
        case let .sent(hashes, transactions):
            hashes.forEach { request.delegate?(.success($0)) }
            if case let .transfer(asset) = request.data.inputType, asset.map().chain == .bitcoinSv, let transaction = transactions.first {
                bsvReceipt = BsvSendReceipt(
                    recipient: transaction.to,
                    amountText: ValueFormatter.full.string(BigInt(transaction.value), asset: asset.map()),
                    transactionId: TransactionId(core: transaction.id).hash,
                )
            }
        }
    }
}
