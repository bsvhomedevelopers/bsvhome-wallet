import Foundation
import BsvHomeCore
import Primitives
import PrimitivesComponents
import PrimitivesTestKit
@testable import Store
import Testing
@testable import Transactions

@MainActor
struct TransactionsViewModelTests {
    @Test
    func failureKeepsCachedHistoryAndRetryUsesTheDisplayedWallet() async {
        let outcomes = HistoryRefreshOutcomes()
        let service = HistoryService { try await outcomes.refresh(walletId: $0) }
        let wallet = Primitives.Wallet.mock(id: .multicoin(address: "displayed-wallet"))
        let model = TransactionsViewModel(service: service, wallet: wallet, type: .all)
        let sections = TransactionViewModel.sections([.mock()])
        model.filterModel.query.value = sections
        #expect(model.state.isLoading)

        await model.load()
        #expect(model.state.isError)
        #expect(model.sections.flatMap(\.values) == sections.flatMap(\.values))

        await model.load()
        #expect(!model.state.isError)
        #expect(!model.state.isLoading)
        #expect(model.sections.flatMap(\.values) == sections.flatMap(\.values))
        #expect(await outcomes.walletIds == [wallet.id.id, wallet.id.id])
    }

    @Test
    func emptyHistoryIsSuccessfulOnlyAfterSynchronization() async {
        let service = HistoryService { _ in }
        let model = TransactionsViewModel(service: service, wallet: .mock(), type: .all)
        #expect(model.state.isLoading)

        await model.load()

        guard case .data = model.state else {
            Issue.record("Expected synchronized empty history")
            return
        }
        #expect(model.sections.isEmpty)
    }

    @Test
    func loadingKeepsCachedRowsWhileSynchronizationIsPending() async {
        let pending = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        let service = HistoryService { _ in
            await withCheckedContinuation { pending.continuation.yield($0) }
        }
        let model = TransactionsViewModel(service: service, wallet: .mock(), type: .all)
        let sections = TransactionViewModel.sections([.mock()])
        model.filterModel.query.value = sections
        let refresh = Task { await model.load() }
        var requests = pending.stream.makeAsyncIterator()
        let completion = await requests.next()

        #expect(model.state.isLoading)
        #expect(model.sections.flatMap(\.values) == sections.flatMap(\.values))
        completion?.resume()
        await refresh.value
        #expect(!model.state.isLoading)
    }
}

private final class HistoryService: BsvHomeTransactionsServiceProtocol, Sendable {
    private let synchronize: @Sendable (String) async throws -> Void

    init(synchronize: @escaping @Sendable (String) async throws -> Void) {
        self.synchronize = synchronize
    }

    func filterChains(wallet _: BsvHomeCore.Wallet) -> [String] {
        [Primitives.Chain.bitcoinSv.rawValue]
    }

    func getCurrency() -> String {
        "USD"
    }

    func sync(assetId _: String?) async throws {
        Issue.record("History must synchronize the displayed wallet explicitly")
    }

    func syncWallet(walletId: String, assetId _: String?) async throws {
        try await synchronize(walletId)
    }
}

private actor HistoryRefreshOutcomes {
    enum Failure: Error { case unavailable }
    private(set) var walletIds: [String] = []

    func refresh(walletId: String) throws {
        walletIds.append(walletId)
        if walletIds.count == 1 {
            throw Failure.unavailable
        }
    }
}
