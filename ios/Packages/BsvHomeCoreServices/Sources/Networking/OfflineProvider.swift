import Foundation
import BsvHomeCore

public final class OfflineProvider: AlienProvider, Sendable {
    public init() {}

    public func request(target _: AlienTarget) async throws -> AlienResponse {
        throw AlienError.RequestError(msg: "Offline mode prohibits network requests")
    }
}
