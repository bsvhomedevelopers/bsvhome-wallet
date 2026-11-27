// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeSupportServiceProtocol
import Primitives

public extension BsvHomeSupportServiceProtocol {
    func syncMessages(fromTimestamp: Int) async throws {
        try await syncMessages(fromTimestamp: UInt64(fromTimestamp))
    }

    func sendMessage(_ content: SupportMessageContent) async throws {
        switch content {
        case let .text(text):
            try await sendText(content: text)
        case let .image(attachment):
            try await sendImage(image: attachment.data, fileName: attachment.fileName, mimeType: attachment.mimeType)
        }
    }

    func retryMessage(_ message: SupportMessage) async throws {
        try await retryMessage(message: message.map())
    }
}
