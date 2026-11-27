import Foundation

struct BsvColdWalletPageRoute: Hashable, Codable {
    let workflowId: UUID
    let index: Int
    let page: BsvColdWalletRoute
}
