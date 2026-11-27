// Copyright (c). Gem Wallet. All rights reserved.

import Components
import enum BsvHomeCore.BsvHomeImage
import BsvHomeCorePrimitives
import Primitives

struct AssetListItemViewModel: Identifiable {
    private let list: AssetList
    init(list: AssetList) {
        self.list = list
    }

    var id: String {
        list.id
    }

    var name: String {
        list.name
    }

    var count: String {
        String(list.count)
    }

    var image: AssetImage {
        AssetImage(type: .text(list.name), imageURL: BsvHomeImage.assetList(listId: list.id).imageURL)
    }
}
