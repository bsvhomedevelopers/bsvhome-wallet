import Foundation
import Primitives

struct PortalSwapDestination: Hashable, Codable {
    let entryAsset: AssetId
    let id: UUID
}
