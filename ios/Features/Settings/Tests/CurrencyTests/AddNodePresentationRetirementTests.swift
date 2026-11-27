import Foundation
import BsvHomeCore
@testable import Settings
import Testing

@MainActor
struct AddNodePresentationRetirementTests {
    @Test
    func anOldImportCompletionCannotDismissTheReplacementForm() async throws {
        let pending = AsyncStream<CheckedContinuation<Void, any Error>>.makeStream()
        let service = service()
        service.setAddNodeReply { _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let parent = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        parent.onPresentImportNode()
        let first = try #require(parent.presentedImportNode)
        first.setInput("https://first.example.invalid")
        await first.load()
        try #require(first.canImport)
        let complete = { parent.onDismissImportNode(id: first.id) }
        let importing = Task {
            try await first.importFoundNode()
            complete()
        }
        var iterator = pending.stream.makeAsyncIterator()
        let reply = try #require(await iterator.next())
        var released = false
        defer {
            if !released {
                reply.resume(throwing: CancellationError())
            }
        }
        parent.onDismissImportNode(id: first.id)
        try await (#require(parent.operationTask)).value
        parent.onPresentImportNode()
        let replacement = try #require(parent.presentedImportNode)
        try #require(replacement.id != first.id)
        reply.resume()
        released = true
        try await importing.value
        #expect(parent.presentedImportNode === replacement)
        #expect(service.addedNodeURLs == ["https://first.example.invalid"])
    }

    @Test
    func theCurrentImportCompletionClosesTheActualParentForm() async throws {
        let service = service()
        let parent = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        parent.onPresentImportNode()
        let child = try #require(parent.presentedImportNode)
        child.setInput("https://first.example.invalid")
        await child.load()
        try await child.importFoundNode()
        parent.onDismissImportNode(id: child.id)
        #expect(parent.presentedImportNode == nil)
        try await (#require(parent.operationTask)).value
        #expect(service.nodeReadCount == 1)
    }

    @Test
    func anOldSheetDismissalCannotRetireOrRefreshTheNewItem() async throws {
        let service = service()
        let parent = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        parent.onPresentImportNode()
        let first = try #require(parent.presentedImportNode)
        parent.onDismissImportNode(id: first.id)
        try await (#require(parent.operationTask)).value
        parent.onPresentImportNode()
        let current = try #require(parent.presentedImportNode)
        parent.onDismissImportNode(id: first.id)
        #expect(parent.presentedImportNode === current)
        #expect(parent.operationTask == nil)
        #expect(service.nodeReadCount == 1)
        parent.onDismissImportNode(id: current.id)
        try await (#require(parent.operationTask)).value
        #expect(parent.presentedImportNode == nil)
        #expect(service.nodeReadCount == 2)
    }

    @Test
    func reopeningAnAlreadyPresentedItemPreservesItsCurrentInput() throws {
        let parent = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service())
        parent.onPresentImportNode()
        let current = try #require(parent.presentedImportNode)
        current.setInput("https://current.example.invalid")
        parent.onPresentImportNode()
        #expect(parent.presentedImportNode === current)
        #expect(current.urlInputModel.text == "https://current.example.invalid")
    }

    private func service() -> ChainSettingsServiceMock {
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { url in
            BsvHomeNodeCheck(url: url, chainId: "bitcoinsv", latestBlockNumber: 123, isInSync: true, latency: Latency(latencyType: .fast, value: 1))
        }
        return service
    }
}
