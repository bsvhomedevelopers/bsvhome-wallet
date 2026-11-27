// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomePerpetualSubscription
import Primitives

public protocol PerpetualObservable: Actor {
    var chartService: ChartObserverService { get }
    func setup(for wallet: Wallet) async
    func disconnect() async
    func subscribe(_ subscription: BsvHomePerpetualSubscription) async throws
    func unsubscribe(_ subscription: BsvHomePerpetualSubscription) async throws
}
