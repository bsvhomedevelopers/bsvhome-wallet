import Components
import Foundation
import BsvHomeCore
@testable import Settings
import Testing

@MainActor
struct AddNodeInputBindingTests {
    @Test
    func editedInputCannotImportThePreviouslyCheckedNodeBeforeItsChangeObserverRuns() async {
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { Self.check($0) }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("https://first.example.invalid")
        await model.load()
        #expect(model.state.value?.url == "https://first.example.invalid")
        model.urlInputModel.text = "https://second.example.invalid"
        await #expect(throws: CancellationError.self) { try await model.importFoundNode() }
        #expect(service.addedNodeURLs.isEmpty)
    }

    @Test
    func changingTheCheckTriggerImmediatelyRemovesTheOldImportableResult() async {
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { Self.check($0) }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("https://first.example.invalid")
        await model.load()
        model.setInput("https://second.example.invalid")
        #expect(model.state.isNoData)
        #expect(model.state.value == nil)
    }

    @Test(arguments: [false, true])
    func aLateOldCheckCannotReplaceTheNewInputResult(fails: Bool) async throws {
        let pending = AsyncStream<CheckedContinuation<BsvHomeNodeCheck, any Error>>.makeStream()
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { url in
            if url == "https://first.example.invalid" {
                return try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
            }
            return Self.check(url)
        }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("https://first.example.invalid")
        let previous = Task { await model.load() }
        var iterator = pending.stream.makeAsyncIterator()
        let reply = try #require(await iterator.next())
        var released = false
        defer {
            if !released {
                reply.resume(throwing: CancellationError())
            }
        }
        model.setInput("https://second.example.invalid")
        await model.load()
        try #require(model.state.value?.url == "https://second.example.invalid")
        if fails {
            reply.resume(throwing: BsvHomeServiceError.Store(msg: "Rejected stale check"))
        } else {
            reply.resume(returning: Self.check("https://first.example.invalid"))
        }
        released = true
        await previous.value
        #expect(model.state.value?.url == "https://second.example.invalid")
    }

    @Test
    func cancellingTheCheckDoesNotMakeItsReturnedNodeImportable() async throws {
        let pending = AsyncStream<CheckedContinuation<BsvHomeNodeCheck, any Error>>.makeStream()
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { _ in
            try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
        }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("https://first.example.invalid")
        let check = Task { await model.load() }
        var iterator = pending.stream.makeAsyncIterator()
        let reply = try #require(await iterator.next())
        check.cancel()
        reply.resume(returning: Self.check("https://first.example.invalid"))
        await check.value
        #expect(model.state.value == nil)
        #expect(model.state.isNoData)
        #expect(service.addedNodeURLs.isEmpty)
    }

    @Test
    func aCheckedOriginalInputImportsTheCoreCanonicalURL() async throws {
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { _ in Self.check("https://first.example.invalid") }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("first.example.invalid/")
        await model.load()
        try #require(model.canImport)
        try await model.importFoundNode()
        #expect(service.checkedNodeURLs == ["first.example.invalid/"])
        #expect(service.addedNodeURLs == ["https://first.example.invalid"])
    }

    @Test
    func aLateFirstAResultCannotReplaceANewerCheckAfterInputReturnsToA() async throws {
        let pending = AsyncStream<CheckedContinuation<BsvHomeNodeCheck, any Error>>.makeStream()
        let service = ChainSettingsServiceMock()
        service.setCheckNodeReply { [weak service] url in
            guard let service else { throw CancellationError() }
            if url == "https://first.example.invalid", service.checkedNodeURLs.filter({ $0 == url }).count == 1 {
                return try await withCheckedThrowingContinuation { pending.continuation.yield($0) }
            }
            var result = Self.check(url)
            result.latestBlockNumber = 456
            return result
        }
        let model = AddNodeSceneViewModel(chain: .bitcoinSv, service: service)
        model.setInput("https://first.example.invalid")
        let old = Task { await model.load() }
        var iterator = pending.stream.makeAsyncIterator()
        let reply = try #require(await iterator.next())
        var released = false
        defer {
            if !released {
                reply.resume(throwing: CancellationError())
            }
        }
        model.setInput("https://second.example.invalid")
        await model.load()
        model.setInput("https://first.example.invalid")
        await model.load()
        let latestBlock = try #require(model.state.value?.latestBlockField.value.text)
        let oldBlock = AddNodeResultViewModel(result: Self.check("https://first.example.invalid")).latestBlockField.value.text
        try #require(latestBlock != oldBlock)
        reply.resume(returning: Self.check("https://first.example.invalid"))
        released = true
        await old.value
        #expect(model.state.value?.latestBlockField.value.text == latestBlock)
        #expect(model.canImport)
    }

    private nonisolated static func check(_ url: String) -> BsvHomeNodeCheck {
        BsvHomeNodeCheck(url: url, chainId: "bitcoinsv", latestBlockNumber: 123, isInSync: true, latency: Latency(latencyType: .fast, value: 1))
    }
}
