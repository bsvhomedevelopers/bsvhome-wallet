import Foundation
import enum BsvHomeCore.BsvHomeServiceError
import Localization

extension BsvHomeServiceError: @retroactive LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .Api(message),
             let .Gateway(message),
             let .Store(message),
             let .Core(message),
             let .Platform(message),
             let .InvalidInput(message),
             let .NotFound(message): message
        case let .Unsupported(message):
            switch message {
            case "ASSET_REQUIRED:TRX": Localized.Errors.notSupported
            default: message.hasPrefix("BSV_API_UNSUPPORTED:") ? Localized.Errors.Connections.unsupportedChain : message
            }
        case let .WalletImport(error): error.localizedDescription
        case .Cancelled: Localized.Errors.cancelled
        }
    }
}
