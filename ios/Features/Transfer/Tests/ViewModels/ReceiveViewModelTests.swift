import CoreImage
import Foundation
import struct BsvHomeCore.BsvHomeBsvAddressRecord
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Testing
@testable import Transfer

@MainActor
struct ReceiveViewModelTests {
    private let initialAddress = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA"
    private let nextAddress = "1cMh228HTCiwS8ZsaakH8A8wze1JR5ZsP"

    @Test
    func compactLayoutKeepsTheSameQrCopyAndShareAddress() async throws {
        let model = makeModel(service: BsvHomeReceiveServiceMock())
        await model.onLoadImage()
        let regularSize = model.qrSize

        await model.onChangeSize(isCompact: true)

        #expect(model.qrSize < regularSize)
        #expect(try decodedAddress(model) == initialAddress)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(model.activityItems(qrImage: model.renderedImage).last as? String == initialAddress)
        #expect(model.canShareAddress)
    }

    @Test
    func completedHdDiscoveryKeepsContinueRecoveryAvailable() async throws {
        let service = BsvHomeReceiveServiceMock()
        service.snapshot = .mock(currentAddress: initialAddress, receiveScanned: 20, changeScanned: 20, selectedIndex: 0, discovery: .complete)
        let model = makeModel(service: service)
        await model.onLoadImage()

        #expect(model.showsBsvRecovery)
        let recovery = try #require(model.onContinueBsvScan())
        await recovery.value

        #expect(service.continueCalls == 1)
        #expect(model.bsvAddressSnapshot?.selectedIndex == 0)
        #expect(model.showsBsvRecovery)
        #expect(try decodedAddress(model) == initialAddress)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(model.canShareAddress)
    }

    @Test
    func leavingBeforeRecoveryStartsCannotContinueScanning() async {
        let service = BsvHomeReceiveServiceMock()
        service.snapshot = .mock(discovery: .incomplete)
        let model = makeModel(service: service)
        await model.onLoadImage()
        let recovery = model.onContinueBsvScan()

        model.onCancelBsvScan()
        await recovery?.value

        #expect(service.continueCalls == 0)
        #expect(!model.isRefreshing)
        await model.onContinueBsvScan()?.value
        #expect(service.continueCalls == 1)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(model.canShareAddress)
    }

    @Test
    func pendingRetryCannotBeCancelledByContinueRecovery() async throws {
        let service = BsvHomeReceiveServiceMock()
        let snapshot = BsvHomeBsvAddressSnapshot.mock(discovery: .incomplete)
        service.snapshot = snapshot
        let model = makeModel(service: service)
        await model.onLoadImage()
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        service.prepareHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let retry = model.onRetryImage()
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()

        #expect(model.receiveState.isLoading)
        #expect(model.onContinueBsvScan() == nil)
        #expect(service.continueCalls == 0)
        completion?.resume(returning: snapshot)
        await retry.value

        #expect(!retry.isCancelled)
        #expect(!model.receiveState.isError)
        #expect(service.prepareCalls == 2)
        #expect(try decodedAddress(model) == initialAddress)
        await model.onContinueBsvScan()?.value
        #expect(service.continueCalls == 1)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(model.canShareAddress)
    }

    @Test
    func bsvReceiptUpdatesTextCopyAndQRCodeTogether() async throws {
        let service = BsvHomeReceiveServiceMock()
        let model = makeModel(service: service)
        #expect(model.address.isEmpty)
        #expect(!model.canShareAddress)

        await model.onLoadImage()
        #expect(model.address == initialAddress)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(try decodedAddress(model) == initialAddress)
        model.onCopyAddress()

        service.snapshot = .mock(records: [
            BsvHomeBsvAddressRecord(branch: .receive, index: 0, address: initialAddress, status: .used, isCurrent: false, isMain: true),
            BsvHomeBsvAddressRecord(branch: .receive, index: 1, address: nextAddress, status: .unused, isCurrent: true, isMain: false),
        ], currentAddress: nextAddress)
        await model.refreshBsvAddress()
        #expect(model.address == nextAddress)
        #expect(model.copyModel.copyValue == nextAddress)
        #expect(try decodedAddress(model) == nextAddress)
        #expect(model.canShareAddress)
        model.isPresentingCopyToast = false
        model.onCopyAddress()
        #expect(model.isPresentingCopyToast)
    }

    @Test
    func bsvAuthenticationCancellationOffersRetryWithoutIdentityFallback() async throws {
        let service = BsvHomeReceiveServiceMock()
        service.prepareHandler = { throw CancellationError() }
        let model = makeModel(service: service)
        await model.onLoadImage()
        #expect(model.receiveState.isError)
        #expect(model.address.isEmpty)
        #expect(model.renderedImage == nil)
        model.onCopyAddress()
        model.onShareSheet()
        #expect(model.presentation == nil)

        service.prepareHandler = nil
        await model.onLoadImage()
        #expect(model.address == initialAddress)
        #expect(try decodedAddress(model) == initialAddress)
    }

    @Test
    func recoveryPauseDoesNotProduceAnEmptyAddressQRCode() async {
        let service = BsvHomeReceiveServiceMock()
        service.snapshot = .mock(currentAddress: nil, allocationPaused: true)
        let model = makeModel(service: service)
        await model.onLoadImage()
        #expect(model.address.isEmpty)
        #expect(model.renderedImage == nil)
        #expect(!model.canShareAddress)
    }

    @Test(arguments: [WalletType.view, .privateKey])
    func singleAddressWalletDoesNotShowDiscoveryState(type: WalletType) async throws {
        let model = makeModel(service: BsvHomeReceiveServiceMock(), type: type)
        await model.onLoadImage()
        #expect(!model.showsBsvAddressDiscovery)
        #expect(!model.showsBsvRecovery)
        #expect(model.address == initialAddress)
        #expect(model.copyModel.copyValue == initialAddress)
        #expect(try decodedAddress(model) == initialAddress)
        #expect(model.canShareAddress)
    }

    @Test
    func bsvRetryCannotRepeatAnAuthenticationAlreadyInProgress() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeReceiveServiceMock()
        service.prepareHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = makeModel(service: service)
        let preparation = Task { await model.onLoadImage() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        #expect(model.receiveState.isLoading)
        #expect(model.renderedImage == nil)

        service.prepareHandler = { throw AnyError("Repeated authentication") }
        await model.onLoadImage()
        #expect(model.receiveState.isLoading)
        #expect(model.address.isEmpty)

        service.snapshot = .mock(currentAddress: nil, allocationPaused: true)
        completion?.resume(returning: service.snapshot)
        await preparation.value
        #expect(!model.receiveState.isError)
    }

    @Test
    func cancelledPreparationReturningNormallyDoesNotKeepLoading() async {
        let pending = AsyncStream<CheckedContinuation<BsvHomeBsvAddressSnapshot, any Error>>.makeStream()
        let service = BsvHomeReceiveServiceMock()
        service.prepareHandler = {
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = makeModel(service: service)
        let preparation = Task { await model.onLoadImage() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        preparation.cancel()
        completion?.resume(returning: .mock())
        await preparation.value
        #expect(model.receiveState.isError)
        #expect(!model.receiveState.isLoading)
        #expect(model.address.isEmpty)

        service.prepareHandler = nil
        await model.onLoadImage()
        #expect(!model.receiveState.isError)
        #expect(model.address == initialAddress)
    }

    @Test
    func leavingBeforeRetryTaskStartsCannotRequestAuthentication() async {
        let service = BsvHomeReceiveServiceMock()
        let model = makeModel(service: service)
        let retry = model.onRetryImage()
        model.onCancelBsvScan()
        await retry.value
        #expect(service.prepareCalls == 0)
        #expect(model.address.isEmpty)
        #expect(!model.isRefreshing)
    }

    @Test(arguments: [Chain.bitcoinSv, .ethereum])
    func leavingDuringNetworkLookupCannotPrepareTheSelectedNetwork(chain: Chain) async throws {
        let pending = AsyncStream<CheckedContinuation<Primitives.Asset, any Error>>.makeStream()
        let service = BsvHomeReceiveServiceMock()
        service.assetHandler = { _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }.map()
        }
        let accounts = [Account.mock(chain: .bitcoinSv, address: initialAddress), Account.mock(chain: .ethereum)]
        let account = try #require(accounts.first { $0.chain == chain })
        let model = ReceiveViewModel(assetData: .mock(asset: chain.asset, account: account), wallet: .mock(accounts: accounts), service: service)
        let selected = chain == .bitcoinSv ? Chain.ethereum : .bitcoinSv
        let lookup = try #require(model.onFinishNetworkSelection([ReceiveNetworkItem(assetId: selected.assetId)]))
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()
        model.onCancelBsvScan()
        completion?.resume(returning: selected.asset)
        await lookup.value
        #expect(model.assetModel.asset.chain == chain)
        #expect(service.prepareCalls == 0)
        #expect(!model.isRefreshing)
    }

    private func makeModel(service: BsvHomeReceiveServiceMock, type: WalletType = .multicoin) -> ReceiveViewModel {
        let account = Account.mock(chain: .bitcoinSv, address: initialAddress)
        return ReceiveViewModel(assetData: .mock(asset: Chain.bitcoinSv.asset, account: account), wallet: .mock(type: type, accounts: [account]), service: service)
    }

    private func decodedAddress(_ model: ReceiveViewModel) throws -> String {
        let detector = try #require(CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]))
        let image = try #require(model.renderedImage?.cgImage)
        let code = try #require(detector.features(in: CIImage(cgImage: image)).first as? CIQRCodeFeature)
        return try #require(code.messageString)
    }
}
