// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

@Observable
@MainActor
public final class NavigationPathState {
    private static let encoder = JSONEncoder()
    private static let decoder = JSONDecoder()

    public private(set) var path = NavigationPath()
    @ObservationIgnored private var removalObservers: [UUID: (path: NavigationPath, onRemoval: @MainActor () -> Void)] = [:]
    @ObservationIgnored private var revision: UInt = 0

    public var binding: Binding<NavigationPath> {
        Binding(
            get: { self.path },
            set: { self.replacePath($0) },
        )
    }

    public var count: Int {
        path.count
    }

    public var isEmpty: Bool {
        path.isEmpty
    }

    public init() {}

    @discardableResult
    public func observeRemoval(onRemoval: @MainActor @escaping () -> Void) -> UUID {
        let id = UUID()
        removalObservers[id] = (path, onRemoval)
        return id
    }

    public func removeRemovalObserver(_ id: UUID) {
        removalObservers[id] = nil
    }

    @discardableResult
    public func append(_ value: some Hashable & Codable) -> Bool {
        if isLastElement(value) {
            return false
        }
        var path = path
        path.append(value)
        replacePath(path)
        return true
    }

    public func setPath(_ items: [any Hashable & Codable]) {
        replacePath(items.reduce(into: NavigationPath()) { $0.append($1) })
    }

    public func reset() {
        guard !path.isEmpty else { return }
        replacePath(NavigationPath())
    }

    public func removeLast(_ k: Int = 1) {
        guard k > 0, !path.isEmpty else { return }
        var path = path
        path.removeLast(min(path.count, k))
        replacePath(path)
    }
}

// MARK: - Private

extension NavigationPathState {
    private func replacePath(_ path: NavigationPath) {
        let currentRevision = revision
        let removed = removalObservers.filter { _, observer in
            guard path.count >= observer.path.count else { return true }
            var prefix = path
            prefix.removeLast(path.count - observer.path.count)
            return prefix != observer.path
        }
        for id in removed.keys {
            removalObservers[id] = nil
        }
        for observer in removed.values {
            observer.onRemoval()
        }
        guard revision == currentRevision else { return }
        self.path = path
        revision &+= 1
    }

    private func encodedElements(_ codable: NavigationPath.CodableRepresentation) -> [String] {
        guard let data = try? Self.encoder.encode(codable),
              let elements = try? Self.decoder.decode([String].self, from: data)
        else {
            return []
        }
        return elements
    }

    private func isLastElement<T: Hashable & Codable>(_ value: T) -> Bool {
        guard !path.isEmpty, let currentCodable = path.codable else { return false }

        let currentElements = encodedElements(currentCodable)

        guard currentElements.count >= 2,
              currentElements[0] == String(reflecting: T.self),
              let data = currentElements[1].data(using: .utf8),
              let lastValue = try? Self.decoder.decode(T.self, from: data)
        else {
            return false
        }

        return lastValue == value
    }
}
