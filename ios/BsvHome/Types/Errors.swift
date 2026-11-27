// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import BsvHomeCore
import Localization
import Primitives

extension KeystoreError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .missingPassword: Localized.Errors.keystoreAccess
        }
    }
}

extension BsvHomeCore.GatewayError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .Offline: Localized.Errors.networkOffline
        case let .NetworkError(string): string
        case let .PlatformError(string): string
        case .NetworkIdMismatch: Localized.Errors.invalidNetworkId
        }
    }
}

extension BsvHomeCore.BsvHomeAddNodeError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .InvalidUrl: Localized.Errors.invalidUrl
        case .InvalidNetworkId: Localized.Errors.invalidNetworkId
        case let .Gateway(error): error.errorDescription
        }
    }
}

extension BsvHomeCore.BsvHomeCoreError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .AnyError(string): string
        case let .SignerError(_, msg): msg
        case .Cancelled: Localized.Errors.cancelled
        }
    }
}

extension BsvHomeCore.BsvHomePaymentError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .NoPaymentOptions: Localized.Errors.notSupported
        case let .InvalidRequest(reason), let .Network(reason): reason
        }
    }
}

extension BsvHomeCore.BsvHomeWalletConnectError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .UnsupportedChains: Localized.Errors.Connections.unsupportedChain
        case .InvalidOrigin: Localized.Errors.Connections.maliciousOrigin
        case .UnsupportedWallets: Localized.Errors.Connections.noSupportedWallets
        case let .Service(msg): msg
        }
    }
}

extension BsvHomeCore.SwapperError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .NotSupportedChain, .NotSupportedAsset:
            Localized.Errors.Swap.notSupportedAsset
        case .NoQuoteAvailable, .NoAvailableProvider, .InvalidRoute,
             .ComputeQuoteError, .TransactionError:
            Localized.Errors.Swap.noQuoteAvailable
        case .InputAmountError: Localized.Errors.Swap.amountTooSmall
        }
    }
}

extension BsvHomeCore.AlienError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .RequestError(msg: msg): msg
        case let .ResponseError(msg: msg): msg
        case let .Http(status, _): Localized.Errors.networkError(status)
        case .Offline: Localized.Errors.networkOffline
        }
    }
}
