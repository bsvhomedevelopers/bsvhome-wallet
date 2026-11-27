@testable import BsvMultisig
import Components
import CoreImage
import Foundation
import BsvHomeCore
import Localization
import Style
import Testing
import UIKit

@MainActor
struct BsvMultisigDataViewModelTests {
    @Test
    func globalPendingProjectionIsNotFilteredAgainUsingThePreferredSignersFlag() {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        var preferred = BsvMultisigFixtures.proposal()
        preferred.personalPending = false
        let context = BsvMultisigListContext(wallets: [BsvMultisigFixtures.wallet()], proposals: [preferred], pendingCount: 1, onOpenWallet: { _ in }, onOpenProposal: { _ in })
        let list = BsvMultisigListView(model: model, context: context)
        #expect(list.pendingProposals == [preferred.id])
    }

    @Test
    func enteringHistoryDuringARecentReadStillLoadsItsOlderPage() async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock()
        let recent = activity()
        service.onRefreshActivity = { _ in await gate.wait(); return recent }
        service.activityRecords["multisig"] = recent
        let model = BsvMultisigViewModel(makeService: { service })
        let loading = Task { await model.refreshActivity("multisig") }
        await gate.started()
        let entered = AsyncStream<Void>.makeStream()
        let history = Task {
            entered.continuation.yield(())
            await model.loadMoreActivity("multisig")
        }
        var events = entered.stream.makeAsyncIterator()
        _ = await events.next()
        await gate.release()
        await loading.value
        await history.value
        #expect(service.calls.filter { $0.hasPrefix("more-activity:") } == ["more-activity:multisig"])
    }

    @Test
    func directoryRefreshDoesNotFetchReceiptsForUnselectedWallets() async {
        let service = BsvMultisigServiceMock()
        service.walletRecords = [BsvMultisigFixtures.wallet(id: "first"), BsvMultisigFixtures.wallet(id: "second")]
        let model = BsvMultisigViewModel(makeService: { service })
        await model.refresh()
        #expect(service.calls == ["load"])
        #expect(model.activities.isEmpty)
    }

    @Test
    func failedOlderPageKeepsCoreRowsAndRetriesTheSamePage() async {
        let service = BsvMultisigServiceMock()
        let first = activity()
        service.activityRecords["multisig"] = first
        let model = BsvMultisigViewModel(makeService: { service })
        await model.refreshActivity("multisig")
        service.operationError = BsvHomeBsvMultisigError.Provider(reason: "untrusted diagnostic")
        service.activityRecords["multisig"]?.isStale = true
        await model.loadMoreActivity("multisig")
        #expect(model.activities["multisig"]?.items == first.items)
        #expect(model.activities["multisig"]?.nextCursor == first.nextCursor)
        #expect(model.activities["multisig"]?.isStale == true)
        #expect(model.activityErrors["multisig"] == Localized.multisigErrorProvider)
        service.operationError = nil
        service.activityRecords["multisig"]?.isStale = false
        service.activityRecords["multisig"]?.nextCursor = nil
        await model.retryActivity("multisig")
        #expect(service.calls.filter { $0 == "more-activity:multisig" }.count == 2)
        #expect(model.activityErrors["multisig"] == nil)
        #expect(model.activities["multisig"]?.nextCursor == nil)
    }

    @Test
    func concurrentWalletReadsCannotReplaceEachOthersActivity() async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock()
        let first = activity(walletId: "first")
        let second = activity(walletId: "second")
        service.onRefreshActivity = { id in
            if id == "first" {
                await gate.wait()
            }
            return id == "first" ? first : second
        }
        let model = BsvMultisigViewModel(makeService: { service })
        let loading = Task { await model.refreshActivity("first") }
        await gate.started()
        await model.refreshActivity("second")
        #expect(model.activities["second"] == second)
        #expect(service.calls.filter { $0 == "refresh-activity:first" }.count == 1)
        await gate.release()
        await loading.value
        #expect(model.activities["first"] == first)
        #expect(model.activities["second"] == second)
    }

    @Test(arguments: [false, true])
    func cancelledViewWaiterDoesNotCancelTheModelsPublicRead(paymail: Bool) async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock()
        let snapshot = activity()
        service.onRefreshActivity = { _ in await gate.wait(); return snapshot }
        service.onLoadPaymail = { id in await gate.wait(); return BsvMultisigFixtures.paymail(walletId: id) }
        let model = BsvMultisigViewModel(makeService: { service })
        let waiter = Task {
            if paymail {
                await model.loadPaymail("multisig")
            } else {
                await model.refreshActivity("multisig")
            }
        }
        await gate.started()
        waiter.cancel()
        let entered = AsyncStream<Void>.makeStream()
        let reopened = Task {
            entered.continuation.yield(())
            if paymail {
                await model.loadPaymail("multisig")
            } else {
                await model.refreshActivity("multisig")
            }
        }
        var events = entered.stream.makeAsyncIterator()
        _ = await events.next()
        await gate.release()
        await waiter.value
        await reopened.value
        if paymail {
            #expect(model.paymailSettings["multisig"] == BsvMultisigFixtures.paymail())
            #expect(service.calls == ["load-paymail:multisig"])
        } else {
            #expect(model.activities["multisig"] == snapshot)
            #expect(service.calls == ["refresh-activity:multisig"])
        }
    }

    @Test(arguments: ["de_DE", "fr_FR"])
    func receivingPaymentAmountUsesTheExistingLocaleEncoding(locale: String) async throws {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service }, locale: Locale(identifier: locale))
        let payload = try await model.paymentPayload("multisig", amount: "0,00000001")
        #expect(try parseBsvMultisigPaymentRequest(value: payload).amount == "0.00000001")
        #expect(service.calls == ["payment-request:multisig:0.00000001"])
    }

    @Test(arguments: [false, true])
    func cancelBeforeReadFirstPollDoesNotConstructService(paymail: Bool) async {
        let service = BsvMultisigServiceMock()
        let constructions = Counter()
        let model = BsvMultisigViewModel(makeService: { constructions.increment(); return service })
        let task = Task {
            if paymail {
                await model.loadPaymail("multisig")
            } else {
                await model.refreshActivity("multisig")
            }
        }
        task.cancel()
        await task.value
        #expect(constructions.count == 0)
        #expect(service.calls.isEmpty)
        #expect(model.activities.isEmpty && model.paymailSettings.isEmpty)
    }

    @Test(arguments: [false, true])
    func cancellationRejectsLatePublicReadWithoutPublishingErrors(paymail: Bool) async {
        let gate = BsvMultisigAsyncGate()
        let service = BsvMultisigServiceMock()
        let snapshot = activity()
        service.onRefreshActivity = { _ in await gate.wait(); return snapshot }
        service.onLoadPaymail = { id in await gate.wait(); return BsvMultisigFixtures.paymail(walletId: id) }
        let model = BsvMultisigViewModel(makeService: { service })
        let task = Task {
            if paymail {
                await model.loadPaymail("multisig")
            } else {
                await model.refreshActivity("multisig")
            }
        }
        await gate.started()
        model.cancel()
        await gate.release()
        await task.value
        #expect(model.activities.isEmpty && model.paymailSettings.isEmpty)
        #expect(model.activityErrors.isEmpty && model.paymailErrors.isEmpty)
        #expect(model.activityLoading.isEmpty && model.paymailLoading.isEmpty)
        #expect(service.calls.contains("cancel"))
    }

    @Test
    func aReadOnlyPaymailSnapshotCannotBeSubmitted() async throws {
        let service = BsvMultisigServiceMock()
        service.onLoadPaymail = { id in BsvMultisigFixtures.paymail(walletId: id, canManage: false) }
        let model = BsvMultisigViewModel(makeService: { service })
        await model.loadPaymail("multisig")
        let snapshot = try #require(model.paymailSettings["multisig"])
        #expect(await model.updatePaymail(snapshot, alias: "another", active: false)?.value == nil)
        #expect(!service.calls.contains { $0.hasPrefix("update-paymail:") })
    }

    @Test
    func aFailedPaymailMutationRequiresANewStatusSnapshot() async throws {
        let service = try await BsvMultisigServiceMock.scoped()
        let model = BsvMultisigViewModel(makeService: { service })
        await model.loadPaymail("multisig")
        let original = try #require(model.paymailSettings["multisig"])
        service.onUpdatePaymail = { _, _, _ in throw BsvHomeBsvMultisigError.ConcurrentUpdate }
        #expect(await model.updatePaymail(original, alias: "updated", active: false)?.value == nil)
        #expect(model.paymailSettings["multisig"] == nil)
        #expect(model.errorMessage == Localized.multisigErrorConcurrentUpdate)
        #expect(await model.updatePaymail(original, alias: "updated", active: false)?.value == nil)
        #expect(service.calls.filter { $0.hasPrefix("update-paymail:") }.count == 1)
        service.onLoadPaymail = { id in BsvMultisigFixtures.paymail(walletId: id, version: 4) }
        await model.loadPaymail("multisig")
        #expect(await model.updatePaymail(original, alias: "updated", active: false)?.value == nil)
        service.onUpdatePaymail = nil
        let fresh = try #require(model.paymailSettings["multisig"])
        #expect(await model.updatePaymail(fresh, alias: "updated", active: false)?.value != nil)
        #expect(model.paymailSettings["multisig"]?.handle == "updated@paymail.bsvhome.com")
        #expect(model.paymailSettings["multisig"]?.active == false)
    }

    @Test
    func cancelledPaymailMutationCannotPublishLateSuccess() async throws {
        let gate = BsvMultisigAsyncGate()
        let service = try await BsvMultisigServiceMock.scoped()
        service.onUpdatePaymail = { settings, _, _ in await gate.wait(); return settings }
        let model = BsvMultisigViewModel(makeService: { service })
        await model.loadPaymail("multisig")
        let snapshot = try #require(model.paymailSettings["multisig"])
        let writing = try #require(model.updatePaymail(snapshot, alias: "updated", active: false))
        await gate.started()
        model.cancel()
        await gate.release()
        #expect(await writing.value == nil)
        #expect(model.paymailSettings.isEmpty && model.paymailErrors.isEmpty)
        #expect(!model.isWorking)
    }

    @Test(arguments: ["", "0.00000001"])
    func canonicalPaymentCodeDecodesAtTheActualDisplaySize(amount: String) async throws {
        let service = BsvMultisigServiceMock()
        let model = BsvMultisigViewModel(makeService: { service })
        let payload = try await model.paymentPayload("multisig", amount: amount)
        let request = try parseBsvMultisigPaymentRequest(value: payload)
        #expect(try request == (BsvMultisigFixtures.paymentRequest(amount: amount)))
        let side = Sizing.wowPay.multisigPaymentQR
        let image = try #require(await QRCodeGenerator().generate(from: payload, size: CGSize(width: side, height: side), correctionLevel: .medium))
        let cgImage = try #require(image.cgImage)
        let detector = try #require(CIDetector(ofType: CIDetectorTypeQRCode, context: CIContext(options: [.useSoftwareRenderer: true]), options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]))
        let decoded = detector.features(in: CIImage(cgImage: cgImage)).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first
        #expect(decoded == payload)
    }

    @Test
    func unknownBackendTextDoesNotBecomeAnInlineError() async {
        let service = BsvMultisigServiceMock()
        service.operationError = BsvHomeBsvMultisigError.Api(code: "private-backend-diagnostic")
        let model = BsvMultisigViewModel(makeService: { service })
        await model.loadPaymail("multisig")
        #expect(model.paymailErrors["multisig"] == Localized.Errors.errorOccurred)
    }

    private func activity(walletId: String = "multisig") -> BsvHomeBsvMultisigActivity {
        BsvHomeBsvMultisigActivity(walletId: walletId, items: [BsvHomeBsvMultisigActivityItem(transactionId: String(repeating: "a", count: 64), direction: .received, proposalId: nil, valueSatoshis: "1", feeSatoshis: nil, memo: "", confirmed: false, timestamp: nil, senderAddresses: [], senderScriptHashes: [String(repeating: "b", count: 64)], recipientAddress: nil, recipientScriptHash: String(repeating: "c", count: 64))], nextCursor: "page-two", isStale: false)
    }

    private final class Counter: @unchecked Sendable {
        private let lock = NSLock()
        private var value = 0
        var count: Int { lock.withLock { value } }

        func increment() {
            lock.withLock { value += 1 }
        }
    }
}
