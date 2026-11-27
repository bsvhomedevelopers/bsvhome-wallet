// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.AddressName
import typealias BsvHomeCore.Chain
import protocol BsvHomeCore.BsvHomeAddressStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreAddressStore: BsvHomeAddressStore, @unchecked Sendable {
    private let store: AddressStore

    public init(store: AddressStore) {
        self.store = store
    }

    public func getAddressName(chain: BsvHomeCore.Chain, address: String) throws -> BsvHomeCore.AddressName? {
        try store.getAddressName(chain: Primitives.Chain(id: chain), address: address).map { $0.map() }
    }

    public func saveAddressNames(names: [BsvHomeCore.AddressName]) async throws {
        try store.updateAddressNames(names.map { $0.map() })
    }

    public func deleteAddressNames(names: [BsvHomeCore.AddressName]) async throws {
        try store.deleteAddressNames(names.map { $0.map() })
    }
}
