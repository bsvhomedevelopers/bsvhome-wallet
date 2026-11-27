@testable import BsvMultisig
import Components
import Foundation
import BsvHomeCore
import BsvHomeCorePrimitivesTestKit
import Observation
import SwiftUI
import Testing
import UIKit

@MainActor
@Suite(.serialized)
struct BsvMultisigPublicFundingHostedPresentationTests {
    @Test(.timeLimit(.minutes(1)))
    func replacingThePayerModelKeepsTheEditedAmountButANewSourceContextResetsIt() async throws {
        let animations = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(animations) }
        let firstFixture = try await BsvMultisigActionScopeFixture.make()
        let nextFixture = try await BsvMultisigActionScopeFixture.make()
        let newFixture = try await BsvMultisigActionScopeFixture.make()
        defer {
            firstFixture.cancel()
            nextFixture.cancel()
            newFixture.cancel()
        }
        let request = try await paymentRequest(fixture: firstFixture, amount: "0.00042")
        let first = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: request)), payer: firstFixture.payer, makeService: { firstFixture.service })
        first.startLoading()
        await first.readTask?.value
        #expect(first.recoveryState.value != nil)
        let state = HostState(model: first, request: request)
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first { $0.activationState == .foregroundActive })
        let previousKeyWindow = scene.windows.first { $0.isKeyWindow }
        let window = UIWindow(windowScene: scene)
        window.frame = scene.coordinateSpace.bounds
        window.rootViewController = UIHostingController(rootView: Host(state: state))
        window.makeKeyAndVisible()
        defer {
            first.cancel()
            state.model.cancel()
            window.isHidden = true
            window.rootViewController = nil
            previousKeyWindow?.makeKey()
        }
        await state.appeared.wait()
        window.layoutIfNeeded()
        let formatter = BsvMultisigAmountFormatter()
        let field = try amountField(in: window)
        #expect(field.text == formatter.input("0.00042"))
        try #require(field.becomeFirstResponder())
        field.selectAll(nil)
        field.insertText(formatter.input("0.00063"))
        field.sendActions(for: .editingChanged)
        try #require(field.resignFirstResponder())
        await withCheckedContinuation { continuation in
            DispatchQueue.main.async { continuation.resume() }
        }
        window.layoutIfNeeded()
        let enteredAmount = field.text
        #expect(enteredAmount == formatter.input("0.00063"))
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        let redrawnAmount = try amountField(in: window).text
        #expect(redrawnAmount == formatter.input("0.00063"))

        state.isLoadingPayers = true
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        #expect(textFields(in: window).isEmpty)
        state.isLoadingPayers = false
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        let committedAmount = try amountField(in: window).text
        #expect(committedAmount == formatter.input("0.00063"))

        let next = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: request)), payer: nextFixture.payer, makeService: { nextFixture.service })
        next.startLoading()
        await next.readTask?.value
        #expect(next.recoveryState.value != nil)
        let contextId = state.sourceContextId
        first.cancel()
        state.model = next
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        #expect(state.sourceContextId == contextId)
        let replacementAmount = try amountField(in: window).text
        #expect(replacementAmount == formatter.input("0.00063"))
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        let rebuiltAmount = try amountField(in: window).text
        #expect(rebuiltAmount == formatter.input("0.00063"))

        let newRequest = try await paymentRequest(fixture: newFixture, amount: "0.00084")
        #expect(newRequest.fingerprint == request.fingerprint)
        let newModel = BsvMultisigPublicFundingViewModel(entry: .payment(.paymentRequest(request: newRequest)), payer: newFixture.payer, makeService: { newFixture.service })
        newModel.startLoading()
        await newModel.readTask?.value
        #expect(newModel.recoveryState.value != nil)
        next.cancel()
        state.model = newModel
        state.request = newRequest
        state.sourceContextId = UUID()
        state.revision += 1
        await state.rebuilt.wait()
        window.layoutIfNeeded()
        let newContextAmount = try amountField(in: window).text
        #expect(newContextAmount == formatter.input("0.00084"))
        for fixture in [firstFixture, nextFixture, newFixture] {
            #expect(fixture.networkRequestCount == 0)
            #expect(fixture.credentialAccessCount == 0)
        }
    }

    private func paymentRequest(fixture: BsvMultisigActionScopeFixture, amount: String) async throws -> BsvHomeBsvMultisigPaymentRequest {
        let results = try await fixture.service.fundingResults()
        let result = try #require(results.first { $0.operationId == fixture.operationId })
        let original: BsvHomeBsvMultisigPaymentRequest
        switch result.target {
        case let .public(request): original = request
        case .member:
            throw BsvHomeServiceError.Unsupported(msg: "Expected the public fixture target")
        }
        var request = original
        request.amount = amount
        return try parseBsvMultisigPaymentRequest(value: bsvMultisigPaymentRequestQrPayload(request: request))
    }

    private func amountField(in view: UIView) throws -> UITextField {
        let fields = textFields(in: view)
        #expect(fields.count == 1)
        return try #require(fields.first)
    }

    private func textFields(in view: UIView) -> [UITextField] {
        guard !view.isHidden else { return [] }
        return (view as? UITextField).map { [$0] } ?? view.subviews.flatMap(textFields)
    }

    @Observable
    @MainActor
    final class HostState {
        let navigation = NavigationPathState()
        let appeared = Signal()
        let rebuilt = Signal()
        var model: BsvMultisigPublicFundingViewModel
        var request: BsvHomeBsvMultisigPaymentRequest
        var sourceContextId = UUID()
        var isLoadingPayers = false
        var revision = 0

        init(model: BsvMultisigPublicFundingViewModel, request: BsvHomeBsvMultisigPaymentRequest) {
            self.model = model
            self.request = request
        }
    }

    private struct Host: View {
        @Bindable var state: HostState

        var body: some View {
            NavigationStack(path: state.navigation.binding) {
                BsvMultisigPublicFundingScene(sourceContextId: state.sourceContextId, entry: .payment(.paymentRequest(request: state.request)), eligiblePayers: state.isLoadingPayers ? .loading : .data([
                    BsvHomeBsvMultisigFundingPayer(walletId: state.model.payer.id, name: state.model.payer.name, address: state.model.payer.accounts[0].address),
                ]), model: state.model, onSelectPayer: { _ in }, onRetryPayers: {}, onOpenRecovery: {}, onClose: {})
            }
            .environment(state.navigation)
            .environment(\.scenePhase, .active)
            .onAppear { state.appeared.send() }
            .onChange(of: state.revision) { _, _ in state.rebuilt.send() }
        }
    }

    @MainActor
    final class Signal {
        private let pair = AsyncStream<Void>.makeStream()

        func send() {
            pair.continuation.yield(())
        }

        func wait() async {
            var iterator = pair.stream.makeAsyncIterator()
            _ = await iterator.next()
        }
    }
}
