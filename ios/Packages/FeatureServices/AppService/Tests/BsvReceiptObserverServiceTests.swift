import AppService
import AppServiceTestKit
import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitivesTestKit
import Primitives
import StreamService
import Testing
import WebSocketClient
import WebSocketClientTestKit

@Suite(.timeLimit(.minutes(1)))
struct BsvReceiptObserverServiceTests {
    @Test
    func concurrentConnectOpensOnlyOneSocket() async {
        let opened = AsyncStream<Void>.makeStream()
        let created = Locked(wrappedValue: 0)
        let service = BsvReceiptCoreServiceMock()
        let socket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let observer = BsvReceiptObserverService(
            makeService: { created.wrappedValue += 1; return service },
            makeWebSocket: { _ in socket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { AsyncThrowingStream { _ in } },
        )
        await withTaskGroup(of: Void.self) { group in
            for _ in 0 ..< 10 {
                group.addTask { await observer.connect() }
            }
        }
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        #expect(created.wrappedValue == 1)
        await observer.disconnect()
        #expect(await socket.state == .disconnected)
    }

    @Test
    func disconnectRejectsLateSubscriptionAndEventsAndKeepsUpdatesStopped() async throws {
        let opened = AsyncStream<Void>.makeStream()
        let preparing = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        let cancelled = AsyncStream<Void>.makeStream()
        let created = Locked(wrappedValue: 0)
        let messages = Locked(wrappedValue: [String]())
        let connectedCalls = Locked(wrappedValue: 0)
        let service = BsvReceiptCoreServiceMock(
            onConnected: {
                connectedCalls.wrappedValue += 1
                await withTaskCancellationHandler {
                    await withCheckedContinuation { preparing.continuation.yield($0) }
                } onCancel: {
                    cancelled.continuation.yield(())
                }
                return "stale-subscription"
            },
            onEvent: { messages.wrappedValue.append($0) },
        )
        let socket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let observer = BsvReceiptObserverService(
            makeService: { created.wrappedValue += 1; return service },
            makeWebSocket: { _ in socket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { AsyncThrowingStream { _ in } },
        )
        await observer.connect()
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        await socket.simulateConnected()
        var preparations = preparing.stream.makeAsyncIterator()
        let completion = try #require(await preparations.next())
        await socket.simulateMessage(Data("queued-late-event".utf8))
        let stop = Task { await observer.disconnect() }
        var cancellations = cancelled.stream.makeAsyncIterator()
        _ = await cancellations.next()
        completion.resume()
        await stop.value
        #expect(await socket.state == .disconnected)
        await socket.simulateConnected()
        await socket.simulateMessage(Data("event-after-stop".utf8))
        await observer.update()
        #expect(created.wrappedValue == 1)
        #expect(connectedCalls.wrappedValue == 1)
        #expect(messages.wrappedValue.isEmpty)
        #expect(await socket.getSentData().isEmpty)
    }

    @Test
    func walletReplacementWaitsForOldWorkerAndSocketTeardown() async throws {
        let opened = AsyncStream<Void>.makeStream()
        let worker = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        let cancelled = AsyncStream<Void>.makeStream()
        let created = Locked(wrappedValue: 0)
        let oldWorkerEnded = Locked(wrappedValue: false)
        let oldSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let newSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let oldService = BsvReceiptCoreServiceMock(
            onRun: {
                await withCheckedContinuation { worker.continuation.yield($0) }
                oldWorkerEnded.wrappedValue = true
            },
            onCancel: { cancelled.continuation.yield(()) },
        )
        let newService = BsvReceiptCoreServiceMock(prepare: {
            #expect(oldWorkerEnded.wrappedValue)
            #expect(await oldSocket.state == .disconnected)
            return true
        })
        let observer = BsvReceiptObserverService(
            makeService: {
                created.wrappedValue += 1
                if created.wrappedValue == 1 {
                    return oldService
                }
                #expect(oldWorkerEnded.wrappedValue)
                return newService
            },
            makeWebSocket: { _ in created.wrappedValue == 1 ? oldSocket : newSocket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { AsyncThrowingStream { _ in } },
        )
        await observer.connect()
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        var workers = worker.stream.makeAsyncIterator()
        let completion = try #require(await workers.next())
        await observer.update()
        var cancellations = cancelled.stream.makeAsyncIterator()
        _ = await cancellations.next()
        #expect(created.wrappedValue == 1)
        completion.resume()
        _ = await connections.next()
        #expect(created.wrappedValue == 2)
        #expect(oldWorkerEnded.wrappedValue)
        await observer.disconnect()
    }

    @Test
    func addressChangesRefreshCoreAndConnectedSendsOnlyLatestSubscription() async {
        let opened = AsyncStream<Void>.makeStream()
        let changes = AsyncThrowingStream<Void, any Error>.makeStream()
        let reads = AsyncStream<WebSocketState>.makeStream()
        let sent = AsyncStream<String>.makeStream()
        let revision = Locked(wrappedValue: 0)
        let socket = WebSocketConnectionMock(
            onConnect: { opened.continuation.yield(()) },
            onSend: { sent.continuation.yield(String(decoding: $0, as: UTF8.self)) },
            onStateRead: { reads.continuation.yield($0) },
        )
        let service = BsvReceiptCoreServiceMock(
            onConnected: { "connected-\(revision.wrappedValue)" },
            onRefresh: {
                revision.wrappedValue += 1
                return "refresh-\(revision.wrappedValue)"
            },
        )
        let observer = BsvReceiptObserverService(
            makeService: { service },
            makeWebSocket: { _ in socket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { changes.stream },
        )
        await observer.connect()
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        changes.continuation.yield(())
        var states = reads.stream.makeAsyncIterator()
        #expect(await states.next() == .connecting)
        #expect(revision.wrappedValue == 1)
        #expect(await socket.getSentData().isEmpty)
        await socket.simulateConnected()
        var messages = sent.stream.makeAsyncIterator()
        #expect(await messages.next() == "connected-1")
        changes.continuation.yield(())
        #expect(await messages.next() == "refresh-2")
        #expect(await socket.getSentData().map { String(decoding: $0, as: UTF8.self) } == ["connected-1", "refresh-2"])
        await observer.disconnect()
    }

    @Test
    func connectedSubscriptionWaitsForAddressSubscriptionToFinish() async throws {
        let opened = AsyncStream<Void>.makeStream()
        let changes = AsyncThrowingStream<Void, any Error>.makeStream()
        let refreshing = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        let sent = AsyncStream<String>.makeStream()
        let calls = Locked(wrappedValue: [String]())
        let socket = WebSocketConnectionMock(
            onConnect: { opened.continuation.yield(()) },
            onSend: { sent.continuation.yield(String(decoding: $0, as: UTF8.self)) },
        )
        let service = BsvReceiptCoreServiceMock(
            onConnected: {
                calls.wrappedValue.append("connected")
                return "subscription-B"
            },
            onRefresh: {
                calls.wrappedValue.append("refresh-start")
                await withCheckedContinuation { refreshing.continuation.yield($0) }
                calls.wrappedValue.append("refresh-end")
                return "subscription-A"
            },
        )
        let observer = BsvReceiptObserverService(
            makeService: { service },
            makeWebSocket: { _ in socket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { changes.stream },
        )
        await observer.connect()
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        changes.continuation.yield(())
        var refreshes = refreshing.stream.makeAsyncIterator()
        let completion = try #require(await refreshes.next())
        await socket.simulateConnected()
        completion.resume()
        var messages = sent.stream.makeAsyncIterator()
        #expect(await messages.next() == "subscription-A")
        #expect(await messages.next() == "subscription-B")
        #expect(calls.wrappedValue == ["refresh-start", "refresh-end", "connected"])
        await observer.disconnect()
    }

    @Test
    func unpreparedHdWalletWaitsForAddressCreationBeforeOpeningSocket() async {
        let changes = AsyncThrowingStream<Void, any Error>.makeStream()
        let prepared = AsyncStream<Bool>.makeStream()
        let opened = AsyncStream<Void>.makeStream()
        let ready = Locked(wrappedValue: false)
        let openCount = Locked(wrappedValue: 0)
        let service = BsvReceiptCoreServiceMock(prepare: {
            let value = ready.wrappedValue
            prepared.continuation.yield(value)
            return value
        })
        let socket = WebSocketConnectionMock(onConnect: { openCount.wrappedValue += 1; opened.continuation.yield(()) })
        let observer = BsvReceiptObserverService(
            makeService: { service },
            makeWebSocket: { _ in socket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { changes.stream },
        )
        await observer.connect()
        var preparations = prepared.stream.makeAsyncIterator()
        #expect(await preparations.next() == false)
        #expect(openCount.wrappedValue == 0)
        #expect(await socket.state == .disconnected)
        ready.wrappedValue = true
        changes.continuation.yield(())
        #expect(await preparations.next() == true)
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        #expect(openCount.wrappedValue == 1)
        await observer.disconnect()
    }

    @Test
    func workerFailureClosesAttemptAndRebuildsThroughReconnectPort() async {
        let opened = AsyncStream<Void>.makeStream()
        let started = AsyncStream<Void>.makeStream()
        let failed = AsyncThrowingStream<Void, any Error>.makeStream()
        let retry = AsyncStream<UInt32>.makeStream()
        let created = Locked(wrappedValue: 0)
        let cancelled = Locked(wrappedValue: false)
        let attempts = Locked(wrappedValue: [UInt32]())
        let oldSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let newSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
        let oldService = BsvReceiptCoreServiceMock(
            onRun: {
                started.continuation.yield(())
                for try await _ in failed.stream {}
            },
            onCancel: { cancelled.wrappedValue = true },
        )
        let replacement = BsvReceiptCoreServiceMock(prepare: {
            #expect(cancelled.wrappedValue)
            #expect(await oldSocket.state == .disconnected)
            return true
        })
        let observer = BsvReceiptObserverService(
            makeService: { created.wrappedValue += 1; return created.wrappedValue == 1 ? oldService : replacement },
            makeWebSocket: { _ in created.wrappedValue == 1 ? oldSocket : newSocket },
            reconnection: BsvReceiptReconnectMock(delay: {
                attempts.wrappedValue.append($0)
                retry.continuation.yield($0)
                return 0
            }),
            addressChanges: { AsyncThrowingStream { _ in } },
        )
        await observer.connect()
        var connections = opened.stream.makeAsyncIterator()
        _ = await connections.next()
        var workers = started.stream.makeAsyncIterator()
        _ = await workers.next()
        failed.continuation.finish(throwing: AnyError("worker failure"))
        var retries = retry.stream.makeAsyncIterator()
        #expect(await retries.next() == 0)
        _ = await connections.next()
        #expect(created.wrappedValue == 2)
        #expect(cancelled.wrappedValue)
        await observer.disconnect()
        #expect(attempts.wrappedValue == [0])
    }

    @Test
    func backgroundPausesBeforeReceiptTeardownWithoutStoppingANewerForeground() async throws {
        let calls = Locked(wrappedValue: [String]())
        let resumed = AsyncStream<[String]>.makeStream()
        let opened = AsyncStream<Int>.makeStream()
        let worker = AsyncStream<CheckedContinuation<Void, Never>>.makeStream()
        let cancelled = AsyncStream<[String]>.makeStream()
        let created = Locked(wrappedValue: 0)
        let oldWorkerEnded = Locked(wrappedValue: false)
        let oldSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(1) })
        let newSocket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(2) })
        let oldService = BsvReceiptCoreServiceMock(
            onRun: {
                await withCheckedContinuation { worker.continuation.yield($0) }
                oldWorkerEnded.wrappedValue = true
            },
            onCancel: { cancelled.continuation.yield(calls.wrappedValue) },
        )
        let newService = BsvReceiptCoreServiceMock(prepare: {
            #expect(oldWorkerEnded.wrappedValue)
            #expect(await oldSocket.state == .disconnected)
            return true
        })
        let observer = BsvReceiptObserverService(
            makeService: {
                created.wrappedValue += 1
                return created.wrappedValue == 1 ? oldService : newService
            },
            makeWebSocket: { _ in created.wrappedValue == 1 ? oldSocket : newSocket },
            reconnection: BsvReceiptReconnectMock(),
            addressChanges: { AsyncThrowingStream { _ in } },
        )
        let tracking = BsvHomeTransactionStateServiceMock(
            onPauseTracking: { calls.wrappedValue.append("pause") },
            onResumeTracking: {
                calls.wrappedValue.append("resume")
                resumed.continuation.yield(calls.wrappedValue)
            },
            onStopTracking: { calls.wrappedValue.append("stop") },
        )
        let lifecycle = AppLifecycleService.mock(transactionStateService: tracking, bsvReceiptObserver: observer, environment: .wowPay)
        await lifecycle.handleScenePhase(.active)
        var resumptions = resumed.stream.makeAsyncIterator()
        #expect(await resumptions.next() == ["resume"])
        var connections = opened.stream.makeAsyncIterator()
        #expect(await connections.next() == 1)
        var workers = worker.stream.makeAsyncIterator()
        let completion = try #require(await workers.next())

        let background = Task { await lifecycle.handleScenePhase(.background) }
        var cancellations = cancelled.stream.makeAsyncIterator()
        #expect(await cancellations.next() == ["resume", "pause"])
        #expect(!oldWorkerEnded.wrappedValue)
        let foreground = Task { await lifecycle.handleScenePhase(.active) }
        #expect(await resumptions.next() == ["resume", "pause", "resume"])
        #expect(calls.wrappedValue == ["resume", "pause", "resume"])
        #expect(created.wrappedValue == 1)

        completion.resume()
        await background.value
        await foreground.value
        #expect(calls.wrappedValue == ["resume", "pause", "resume"])
        #expect(await connections.next() == 2)
        #expect(created.wrappedValue == 2)
        await lifecycle.handleScenePhase(.background)
        #expect(calls.wrappedValue == ["resume", "pause", "resume", "pause"])
        #expect(await newSocket.state == .disconnected)
    }

    @Test
    func lifecycleGatesReceiptConnectionAndBackgroundUpdatesStayStopped() async {
        for environment in [BsvHomeRuntimeEnvironment.production, .wowPay] {
            let opened = AsyncStream<Void>.makeStream()
            let created = Locked(wrappedValue: 0)
            let socket = WebSocketConnectionMock(onConnect: { opened.continuation.yield(()) })
            let observer = BsvReceiptObserverService(
                makeService: { created.wrappedValue += 1; return BsvReceiptCoreServiceMock() },
                makeWebSocket: { _ in socket },
                reconnection: BsvReceiptReconnectMock(),
                addressChanges: { AsyncThrowingStream { _ in } },
            )
            let lifecycle = AppLifecycleService.mock(bsvReceiptObserver: observer, environment: environment)
            await lifecycle.handleScenePhase(.active)
            var connections = opened.stream.makeAsyncIterator()
            if environment == .wowPay {
                _ = await connections.next()
            }
            await lifecycle.handleScenePhase(.background)
            await lifecycle.updateWalletConnections()
            #expect(created.wrappedValue == (environment == .wowPay ? 1 : 0))
            #expect(await socket.state == .disconnected)
            await lifecycle.handleScenePhase(.active)
            if environment == .wowPay {
                _ = await connections.next()
            }
            await lifecycle.handleScenePhase(.background)
            #expect(created.wrappedValue == (environment == .wowPay ? 2 : 0))
        }
    }
}
