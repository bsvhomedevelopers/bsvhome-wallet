// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import Primitives
import SwiftHTTPClient

public protocol BsvHomeAPIAssetsService: Sendable {
    func getAssets(currency: String?, assetIds: [AssetId]) async throws -> [AssetBasic]
}

public struct BsvHomeAPIService {
    let provider: Provider<BsvHomeAPI>

    public static let sharedProvider = Provider<BsvHomeAPI>()

    public init(provider: Provider<BsvHomeAPI> = Self.sharedProvider) {
        self.provider = provider
    }
}

extension BsvHomeAPIService: BsvHomeAPIAssetsService {
    public func getAssets(currency: String?, assetIds: [AssetId]) async throws -> [AssetBasic] {
        try await provider
            .request(.getAssets(assetIds, currency: currency))
            .mapResponse(as: [AssetBasic].self)
    }
}

public extension SwiftHTTPClient.Response {
    @discardableResult
    func mapResponse<T: Decodable>(as type: T.Type) throws -> T {
        try mapOrError(as: type, asError: ResponseError.self)
    }
}
