// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeSupportStore
import struct BsvHomeCore.SupportMessage
import struct BsvHomeCore.SupportTyping
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreSupportStore: BsvHomeSupportStore, Sendable {
    private let store: SupportChatStore

    public let typing = ObservableSupportTyping()

    public init(store: SupportChatStore) {
        self.store = store
    }

    public func saveMessages(messages: [BsvHomeCore.SupportMessage]) async throws {
        try store.addMessages(messages.map { $0.map() })
    }

    public func saveMessage(id: String, message: BsvHomeCore.SupportMessage) async throws {
        try store.replace(id: id, with: message.map())
    }

    public func updateTyping(typing: BsvHomeCore.SupportTyping) throws {
        self.typing.update(typing.map())
    }

    public func clearTyping() throws {
        typing.clear()
    }
}
