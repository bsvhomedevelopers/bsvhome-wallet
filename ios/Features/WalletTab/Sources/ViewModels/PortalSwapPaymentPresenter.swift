import Foundation
import enum BsvHomeCore.BsvHomePortalSwapError
import protocol BsvHomeCore.BsvHomePortalSwapServiceProtocol
import Observation
import Primitives

@Observable
@MainActor
public final class PortalSwapPaymentPresenter {
    private enum State {
        case inactive
        case idle
        case preparing(UUID, PortalSwapPaymentSource, any BsvHomePortalSwapServiceProtocol)
        case presented(UUID, PortalSwapPaymentSource, PortalSwapPaymentContext)

        var id: UUID? {
            switch self {
            case .inactive, .idle: nil
            case let .preparing(id, _, _), let .presented(id, _, _): id
            }
        }
    }

    private var state: State = .idle

    public init() {}

    public var payment: PortalSwapPaymentContext? {
        guard case let .presented(_, _, context) = state else { return nil }
        return context
    }

    public var source: PortalSwapPaymentSource? {
        switch state {
        case .inactive, .idle: nil
        case let .preparing(_, source, _), let .presented(_, source, _): source
        }
    }

    public func present(
        wallet: Wallet,
        source: PortalSwapPaymentSource,
        orderId: String,
        service: any BsvHomePortalSwapServiceProtocol,
        isActive: @escaping @MainActor () -> Bool,
    ) async throws {
        guard case .idle = state, isActive() else { throw BsvHomePortalSwapError.PaymentUnavailable }
        let id = UUID()
        state = .preparing(id, source, service)
        defer {
            if state.id == id {
                state = .idle
            }
        }
        try await withTaskCancellationHandler {
            let saved = try await service.savedPayment(orderId: orderId)
            try validate(id, isActive: isActive)
            if let saved {
                let recovery = try await service.recoverPayment(orderId: orderId)
                try validate(id, isActive: isActive)
                if recovery.order.sourceTxid == saved.transactionId {
                    return
                }
                guard recovery.canRebroadcast else { throw BsvHomePortalSwapError.PaymentUnavailable }
            }
            let preparation = try await service.preparePayment(orderId: orderId)
            try validate(id, isActive: isActive)
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, any Error>) in
                do {
                    let context = try PortalSwapPaymentContext(
                        wallet: wallet,
                        preparation: preparation,
                        savedPayment: saved,
                        service: service,
                        completion: { [weak self] result in
                            if self?.state.id == id {
                                self?.state = .idle
                            }
                            continuation.resume(with: result)
                        },
                    )
                    state = .presented(id, source, context)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        } onCancel: {
            Task { @MainActor in
                if self.state.id == id {
                    self.cancel()
                }
            }
        }
    }

    public func cancel() {
        let previous = state
        if case .inactive = previous {
            return
        }
        state = .idle
        switch previous {
        case .inactive, .idle: break
        case let .preparing(_, _, service): service.cancel()
        case let .presented(_, _, context): context.finish(.failure(CancellationError()))
        }
    }

    public func activate() {
        if case .inactive = state {
            state = .idle
        }
    }

    public func deactivate() {
        cancel()
        state = .inactive
    }

    private func validate(_ id: UUID, isActive: @MainActor () -> Bool) throws {
        try Task.checkCancellation()
        guard state.id == id, isActive() else { throw CancellationError() }
    }
}
