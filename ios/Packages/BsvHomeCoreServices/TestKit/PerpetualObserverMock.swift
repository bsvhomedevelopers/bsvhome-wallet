// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomePerpetualSubscription
@testable import BsvHomeCoreServices
import Primitives

public actor PerpetualObserverMock: PerpetualObservable {
    public let chartService = ChartObserverService()

    public private(set) var isConnected: Bool = false

    public init() {}

    public func setup(for _: Wallet) async {
        isConnected = true
    }

    public func disconnect() async {
        isConnected = false
    }


    public func subscribe(_: BsvHomePerpetualSubscription) async throws {}
    public func unsubscribe(_: BsvHomePerpetualSubscription) async throws {}
}
