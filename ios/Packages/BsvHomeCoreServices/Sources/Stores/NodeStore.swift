// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import typealias BsvHomeCore.Chain
import struct BsvHomeCore.Node
import protocol BsvHomeCore.BsvHomeNodeStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreNodeStore: BsvHomeNodeStore, Sendable {
    private let store: NodeStore

    public init(store: NodeStore) {
        self.store = store
    }

    public func getNodes(chain: BsvHomeCore.Chain) async throws -> [BsvHomeCore.Node] {
        try store.nodes(chain: Primitives.Chain(id: chain)).map { $0.node.map() }
    }

    public func addNode(chain: BsvHomeCore.Chain, node: BsvHomeCore.Node) async throws {
        try store.addNodes(chainNodes: [ChainNodes(chain: chain, nodes: [node.map()])])
    }

    public func deleteNode(chain: BsvHomeCore.Chain, url: String) async throws {
        try store.deleteNode(chain: Primitives.Chain(id: chain), url: url)
    }
}
