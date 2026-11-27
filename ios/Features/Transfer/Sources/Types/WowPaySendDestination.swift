import Foundation

public struct WowPaySendDestination: Hashable, Codable {
    public let id: UUID

    public init(id: UUID) {
        self.id = id
    }
}
