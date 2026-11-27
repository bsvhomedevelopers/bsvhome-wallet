// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.Contact
import struct BsvHomeCore.ContactAddress
import protocol BsvHomeCore.BsvHomeContactStore
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreContactStore: BsvHomeContactStore, @unchecked Sendable {
    private let store: ContactStore

    public init(store: ContactStore) {
        self.store = store
    }

    public func getAddresses(contactId: String) async throws -> [BsvHomeCore.ContactAddress] {
        try store.getAddresses(contactId: contactId).map { $0.map() }
    }

    public func saveContact(contact: BsvHomeCore.Contact, addresses: [BsvHomeCore.ContactAddress]) async throws {
        try store.addContact(contact.map(), addresses: addresses.map { $0.map() })
    }

    public func updateContact(contact: BsvHomeCore.Contact, addresses: [BsvHomeCore.ContactAddress], deleteAddressIds: [String]) async throws {
        try store.updateContact(
            contact.map(),
            deleteAddressIds: deleteAddressIds,
            addresses: addresses.map { $0.map() },
        )
    }

    public func deleteContact(contactId: String) async throws {
        try store.deleteContact(id: contactId)
    }
}
