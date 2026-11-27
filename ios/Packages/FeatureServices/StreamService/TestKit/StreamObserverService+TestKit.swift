// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import protocol BsvHomeCore.BsvHomeStreamServiceProtocol
import BsvHomeCorePrimitivesTestKit
import StreamService
import WebSocketClient
import WebSocketClientTestKit

public extension StreamObserverService {
    static func mock(
        service: any BsvHomeStreamServiceProtocol = BsvHomeStreamServiceMock(),
        webSocket: any WebSocketConnectable = WebSocketConnectionMock(),
    ) -> StreamObserverService {
        StreamObserverService(
            service: service,
            webSocket: webSocket,
        )
    }
}
