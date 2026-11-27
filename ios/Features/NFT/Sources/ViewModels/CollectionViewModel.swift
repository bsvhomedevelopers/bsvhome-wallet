// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomeNftServiceProtocol
import BsvHomeCorePrimitives
import Primitives
import PrimitivesComponents
import Store
import SwiftUI

@Observable
@MainActor
public final class CollectionViewModel: CollectionsViewable, Sendable {
    private let service: any BsvHomeNftServiceProtocol
    private let collectionName: String

    public let query: ObservableQuery<NFTRequest>

    public var isPresentingReceiveSelectAssetType: SelectAssetType?

    public init(
        service: any BsvHomeNftServiceProtocol,
        wallet: Wallet,
        collectionId: String,
        collectionName: String,
    ) {
        self.service = service
        self.collectionName = collectionName
        query = ObservableQuery(NFTRequest(walletId: wallet.id, filter: .collection(id: collectionId)), initialValue: [])
    }

    public var title: String {
        collectionName
    }

    public var content: CollectionsContent {
        CollectionsContent(items: service.listItems(data: query.value.map { $0.map() }, list: .collection).map(NFTGridPosterBuilder.item))
    }
}
