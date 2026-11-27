import Components
import Foundation
import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import protocol BsvHomeCore.BsvHomeReceiveServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

@Observable
@MainActor
public final class ReceiveViewModel: Sendable {
    var qrSize: CGFloat {
        isCompact ? Sizing.wowPay.receivePageCompactQR : Sizing.wowPay.receivePageQR
    }

    private var isCompact = false

    private(set) var assetModel: AssetViewModel
    private(set) var address: String
    private(set) var receiveState: StateViewType<Void> = .noData
    private(set) var bsvAddressSnapshot: BsvHomeBsvAddressSnapshot?
    private var bsvPrepared = false
    private var addressRevision: UInt64 = 0
    private var imageLoadTask: Task<Void, Never>?
    private var networkSelectionTask: Task<Void, Never>?
    private(set) var isRefreshing = false

    var presentation: ReceivePresentationType?
    var renderedImage: UIImage?
    var isPresentingAlertMessage: AlertMessage?

    private let wallet: Wallet
    private let service: any BsvHomeReceiveServiceProtocol
    private let generator = QRCodeGenerator()
    let networkAssetIds: [AssetId]

    private init(
        asset: Asset,
        associations: [AssetAssociation],
        wallet: Wallet,
        address: String,
        service: any BsvHomeReceiveServiceProtocol,
    ) {
        assetModel = AssetViewModel(asset: asset)
        self.wallet = wallet
        self.address = asset.chain == .bitcoinSv ? "" : address
        self.service = service
        networkAssetIds = service.networkAssetIds(
            assetId: asset.id.identifier,
            associations: associations.map(\.assetId.identifier),
            wallet: wallet.map(),
        ).map { AssetId(core: $0) }
    }

    public convenience init(assetData: AssetData, wallet: Wallet, service: any BsvHomeReceiveServiceProtocol) {
        self.init(
            asset: assetData.asset,
            associations: assetData.associations,
            wallet: wallet,
            address: assetData.account.address,
            service: service,
        )
    }

    public convenience init(assetAddress: AssetAddress, wallet: Wallet, service: any BsvHomeReceiveServiceProtocol) {
        self.init(
            asset: assetAddress.asset,
            associations: [],
            wallet: wallet,
            address: assetAddress.address,
            service: service,
        )
    }

    var title: String {
        Localized.Receive.title(assetModel.symbol)
    }

    var addressShort: String {
        BsvHomeAddressService.shared.format(address: address, chain: assetModel.asset.chain)
    }

    var copyTitle: String {
        Localized.Wallet.copyAddress
    }

    var isBsv: Bool { assetModel.asset.chain == .bitcoinSv }

    var showsBsvAddressDiscovery: Bool { isBsv && wallet.hasMnemonic }

    var showsBsvRecovery: Bool {
        showsBsvAddressDiscovery && bsvAddressSnapshot != nil
    }

    var canShareAddress: Bool { address.isNotEmpty && renderedImage != nil }

    var warningMessage: String {
        [Localized.Receive.warning(assetModel.symbol.boldMarkdown(), assetModel.networkFullName.boldMarkdown()), memoWarningText, testnetWarningText]
            .compactMap(\.self)
            .joined(separator: " ")
    }

    private var testnetWarningText: String? {
        ChainConfig.config(chain: assetModel.asset.chain).isTestnet ? Localized.shastaTestnetNotice : nil
    }

    private var memoWarningText: String? {
        switch service.memoWarning(chain: assetModel.asset.chain.rawValue) {
        case .destinationTag: Localized.Wallet.Receive.noDestinationTagRequired
        case .memo: Localized.Wallet.Receive.noMemoRequired
        case .notSupported: nil
        }
    }

    var copyModel: CopyTypeViewModel {
        CopyTypeViewModel(
            type: .address(assetModel.asset, address: addressShort),
            copyValue: address,
        )
    }

    var showNetworkSelector: Bool {
        networkAssetIds.count > 1
    }

    var networkSelectorModel: ReceiveNetworkSelectorViewModel {
        ReceiveNetworkSelectorViewModel(
            assetIds: networkAssetIds,
        )
    }

    var isPresentingSheet: ReceivePresentationType? {
        get {
            switch presentation {
            case .share, .networkSelector: presentation
            case .copy, nil: nil
            }
        }
        set {
            presentation = newValue
        }
    }

    var isPresentingCopyToast: Bool {
        get {
            if case .copy = presentation {
                return true
            }
            return false
        }
        set {
            presentation = newValue ? .copy : nil
        }
    }

    func activityItems(qrImage: UIImage?) -> [Any] {
        if let qrImage {
            return [qrImage, address]
        }
        return [address]
    }

    private func enableAsset() async {
        do {
            try await service.enableAsset(walletId: wallet.id.id, assetId: assetModel.asset.id.identifier)
        } catch {
            debugLog("ReceiveViewModel enableAsset error: \(error)")
        }
    }

    private func prefetchAssociations() async {
        do {
            _ = try await service.syncMissingAssets(assetIds: networkAssetIds.filter { $0 != assetModel.asset.id }.ids)
        } catch {
            debugLog("ReceiveViewModel prefetchAssociations error: \(error)")
        }
    }

    private func generateQRCode() async {
        let address = address
        let assetId = assetModel.asset.id
        let size = qrSize
        guard address.isNotEmpty else {
            renderedImage = nil
            return
        }
        let image = await generator.generate(
            from: address,
            size: CGSize(
                width: size,
                height: size,
            ),
            logo: UIImage.name("wowpay-mark"),
        )
        guard !Task.isCancelled, address == self.address, assetId == assetModel.asset.id, size == qrSize else { return }
        renderedImage = image
        if image == nil {
            receiveState = .error(AnyError(Localized.Errors.unknown))
        } else if !isBsv {
            receiveState = .data(())
        }
    }

    private func apply(_ snapshot: BsvHomeBsvAddressSnapshot) async {
        bsvAddressSnapshot = snapshot
        let current = snapshot.currentAddress ?? ""
        if current != address {
            address = current
            renderedImage = nil
        }
        receiveState = .data(())
        await generateQRCode()
    }
}

// MARK: - Actions

extension ReceiveViewModel {
    func onChangeSize(isCompact: Bool) async {
        guard self.isCompact != isCompact else { return }
        self.isCompact = isCompact
        await generateQRCode()
    }

    func onTaskOnce() {
        Task {
            async let enabled: Void = enableAsset()
            async let prefetched: Void = prefetchAssociations()
            _ = await (enabled, prefetched)
        }
    }

    func onSelectNetwork() {
        presentation = .networkSelector
    }

    @discardableResult
    func onFinishNetworkSelection(_ items: [ReceiveNetworkItem]) -> Task<Void, Never>? {
        presentation = nil
        guard let assetId = items.first?.assetId, assetId != assetModel.asset.id else { return nil }

        networkSelectionTask?.cancel()
        let revision = addressRevision
        networkSelectionTask = Task {
            do {
                try Task.checkCancellation()
                let asset = try await service.asset(assetId: assetId.identifier).map()
                try Task.checkCancellation()
                guard addressRevision == revision else { return }
                let account = try wallet.account(for: asset.chain)
                cancelBsvScan()
                assetModel = AssetViewModel(asset: asset)
                address = asset.chain == .bitcoinSv ? "" : account.address
                renderedImage = nil
                bsvPrepared = false
                bsvAddressSnapshot = nil
                receiveState = .noData
                await onLoadImage()
                try Task.checkCancellation()
                await enableAsset()
            } catch {
                guard !Task.isCancelled, addressRevision == revision else { return }
                isPresentingAlertMessage = AlertMessage(error: error)
            }
        }
        return networkSelectionTask
    }

    func onShareSheet() {
        guard canShareAddress else { return }
        presentation = .share
    }

    func onCopyAddress() {
        guard address.isNotEmpty else { return }
        if isPresentingCopyToast {
            copyModel.copy()
        }
        presentation = .copy
    }

    func onLoadImage() async {
        guard isBsv else {
            receiveState = .loading
            await generateQRCode()
            return
        }
        guard !receiveState.isLoading else { return }
        addressRevision &+= 1
        let revision = addressRevision
        let assetId = assetModel.asset.id
        receiveState = .loading
        do {
            try Task.checkCancellation()
            let snapshot = try await service.prepareBsvAddresses(walletId: wallet.id.id)
            guard assetId == assetModel.asset.id, addressRevision == revision else { return }
            try Task.checkCancellation()
            bsvPrepared = true
            await apply(snapshot)
            guard assetId == assetModel.asset.id, addressRevision == revision else { return }
            try Task.checkCancellation()
            await refreshBsvAddress()
        } catch {
            guard assetId == assetModel.asset.id, addressRevision == revision else { return }
            receiveState = .error(error)
        }
    }

    @discardableResult
    func onRetryImage() -> Task<Void, Never> {
        imageLoadTask?.cancel()
        let task = Task { await onLoadImage() }
        imageLoadTask = task
        return task
    }

    @discardableResult
    func onContinueBsvScan() -> Task<Void, Never>? {
        guard showsBsvRecovery, !isRefreshing, !receiveState.isLoading else { return nil }
        imageLoadTask?.cancel()
        let revision = addressRevision
        let task = Task {
            guard !Task.isCancelled, addressRevision == revision else { return }
            await refreshBsvAddress(continueScan: true)
        }
        imageLoadTask = task
        return task
    }

    func refreshBsvAddress(continueScan: Bool = false) async {
        guard isBsv, bsvPrepared, !isRefreshing else { return }
        let assetId = assetModel.asset.id
        let revision = addressRevision
        isRefreshing = true
        defer {
            if addressRevision == revision {
                isRefreshing = false
            }
        }
        do {
            try Task.checkCancellation()
            let generation = try service.bsvScanGeneration(walletId: wallet.id.id)
            let snapshot = if continueScan {
                try await service.continueBsvScan(walletId: wallet.id.id, generation: generation)
            } else {
                try await service.syncBsvAddresses(walletId: wallet.id.id, generation: generation)
            }
            guard assetId == assetModel.asset.id, addressRevision == revision else { return }
            try Task.checkCancellation()
            await apply(snapshot)
        } catch {
            guard assetId == assetModel.asset.id, addressRevision == revision else { return }
            receiveState = .error(error)
        }
    }

    func onCancelBsvScan() {
        networkSelectionTask?.cancel()
        networkSelectionTask = nil
        cancelBsvScan()
    }

    private func cancelBsvScan() {
        addressRevision &+= 1
        imageLoadTask?.cancel()
        imageLoadTask = nil
        guard isBsv else { return }
        if receiveState.isLoading || isRefreshing {
            receiveState = .error(CancellationError())
        }
        isRefreshing = false
        do {
            try service.cancelBsvScan(walletId: wallet.id.id)
        } catch {
            receiveState = .error(error)
        }
    }
}
