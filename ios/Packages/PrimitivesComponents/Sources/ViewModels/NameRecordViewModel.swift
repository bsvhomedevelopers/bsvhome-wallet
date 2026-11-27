// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeNameRecordState
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import BsvHomeCorePrimitives
import Primitives

@Observable
@MainActor
public final class NameRecordViewModel {
    private let nameService: any BsvHomeNameServiceProtocol
    private(set) var nameRecordTask: Task<Void, Never>?

    public var state: BsvHomeNameRecordState = .none

    public init(nameService: any BsvHomeNameServiceProtocol) {
        self.nameService = nameService
    }

    public func getNameRecord(name: String, chain: Chain) {
        guard name != state.requestedName() else { return }
        nameRecordTask?.cancel()

        guard nameService.isNameSupported(name: name) else {
            state = .none
            return
        }

        state = .loading(name: name)
        nameRecordTask = Task {
            do {
                try await Task.sleep(for: .milliseconds(nameService.nameRecordDebounceMilliseconds()))
                let resolved = try await nameService.getNameRecord(name: name, chain: chain)
                guard state == .loading(name: name) else { return }
                state = resolved
            } catch {
                guard !error.isCancelled, state == .loading(name: name) else { return }
                state = .error
            }
        }
    }

    public func reset() {
        nameRecordTask?.cancel()
        state = .none
    }

    public func isNameSupported(name: String) -> Bool {
        nameService.isNameSupported(name: name)
    }
}
