import struct BsvHomeCore.BsvHomePaymailMutation
import struct BsvHomeCore.BsvHomePaymailSnapshot
import enum BsvHomeCore.BsvHomePaymailVerification
import BsvHomeCoreServicesTestKit
import Testing
@testable import WalletTab

@MainActor
struct WowPayPaymailViewModelTests {
    @Test
    func failedRefreshClearsVerifiedHandle() async {
        let snapshot = snapshot(activeHandle: "alice@example.com")
        let service = BsvHomePaymailServiceMock(snapshot: snapshot, mutation: BsvHomePaymailMutation(status: .confirmed, snapshot: snapshot))
        let model = WowPayPaymailViewModel(walletId: snapshot.walletId, makeService: { service })
        await model.load()
        #expect(model.activeHandle == "alice@example.com")
        await service.setLoadError(.InvalidResponse)
        await model.load()
        #expect(model.activeHandle == nil)
        #expect(model.snapshot == nil)
    }

    @Test
    func confirmationPrecedesSigningAndPendingResultBlocksAnotherOperation() async throws {
        let snapshot = snapshot()
        let service = BsvHomePaymailServiceMock(snapshot: snapshot, mutation: BsvHomePaymailMutation(status: .pendingVerification, snapshot: nil))
        let model = WowPayPaymailViewModel(walletId: snapshot.walletId, makeService: { service })
        await model.load()
        model.alias = " Alice "
        model.confirmRegistration()
        let confirmation = try #require(model.confirmation)
        #expect(confirmation.handle == "alice@example.com")
        #expect(await service.intents.isEmpty)
        await model.submit(confirmation)
        #expect(await service.submissionCount == 1)
        #expect(model.operation == .pendingVerification)
        #expect(model.alias == " Alice ")
        #expect(model.activeHandle == nil)
        model.confirmRegistration()
        #expect(model.confirmation == nil)
        await service.setSnapshot(self.snapshot(verification: .pending))
        await model.load()
        #expect(model.operation == .pendingVerification)
        model.confirmRegistration()
        #expect(model.confirmation == nil)
        await service.setSnapshot(self.snapshot(verification: .reconciled))
        await model.load()
        #expect(model.operation == .idle)
    }

    @Test
    func cancellationDiscardsLateBindingResponse() async {
        let gate = AsyncStream<Void>.makeStream()
        let started = AsyncStream<Void>.makeStream()
        let snapshot = snapshot(activeHandle: "alice@example.com")
        let service = BsvHomePaymailServiceMock(snapshot: snapshot, mutation: BsvHomePaymailMutation(status: .confirmed, snapshot: snapshot), onLoad: {
            started.continuation.yield(())
            for await _ in gate.stream {
                break
            }
        })
        let model = WowPayPaymailViewModel(walletId: snapshot.walletId, makeService: { service })
        let task = Task { await model.load() }
        for await _ in started.stream {
            break
        }
        model.cancel()
        gate.continuation.yield(())
        await task.value
        #expect(model.activeHandle == nil)
        #expect(model.snapshot == nil)
    }

    private func snapshot(activeHandle: String? = nil, verification: BsvHomePaymailVerification = .clear) -> BsvHomePaymailSnapshot {
        BsvHomePaymailSnapshot(walletId: "paymail-test-wallet", address: "1BoatSLRHtKNngkdXEeobR76b53LETtpyT", publicOrigin: "https://example.com", domains: ["example.com"], registrationDomains: ["example.com"], bindings: [], activeHandle: activeHandle, verification: verification)
    }
}
