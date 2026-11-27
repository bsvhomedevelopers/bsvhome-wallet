import Foundation

@MainActor
final class PhaseOneServicesFactory {
    private let make: @MainActor @Sendable () throws -> PhaseOneServices
    private var services: PhaseOneServices?
    private enum Suspension {
        case ready
        case pending(UUID, Task<Void, Error>)
        case retry
    }

    private var suspension = Suspension.ready

    init(make: @escaping @MainActor @Sendable () throws -> PhaseOneServices) {
        self.make = make
    }

    func sharedServices() async throws -> PhaseOneServices {
        try await finishSuspension()
        if let services {
            return services
        }
        let services = try make()
        self.services = services
        return services
    }

    private func finishSuspension() async throws {
        if case .retry = suspension {
            suspend()
        }
        if case let .pending(id, task) = suspension {
            do {
                try await task.value
                if case let .pending(current, _) = suspension, current == id {
                    suspension = .ready
                }
            } catch {
                if case let .pending(current, _) = suspension, current == id {
                    suspension = .retry
                }
                throw error
            }
        }
    }

    func suspend() {
        guard let services else { return }
        let previous: Task<Void, Error>? = if case let .pending(_, task) = suspension {
            task
        } else {
            nil
        }
        let cancellation = Task.detached { try services.account.cancel() }
        let task = Task.detached {
            try await cancellation.value
            if let previous {
                try await previous.value
            }
            try await services.mpc.suspend()
        }
        suspension = .pending(UUID(), task)
    }

    func suspendAndWait() async throws {
        suspend()
        try await finishSuspension()
    }
}
