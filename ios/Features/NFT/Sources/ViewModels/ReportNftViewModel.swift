// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitives
import protocol BsvHomeCore.BsvHomeCollectibleServiceProtocol
import Components
import Foundation
import Localization
import BsvHomeCoreServices
import Primitives

@Observable
@MainActor
public final class ReportNftViewModel {
    private let service: any BsvHomeCollectibleServiceProtocol
    private let assetData: NFTAssetData
    private let onComplete: VoidAction

    var state: StateViewType<Bool> = .noData

    let reasons = ReportReasonViewModel.allCases

    init(service: any BsvHomeCollectibleServiceProtocol, assetData: NFTAssetData, onComplete: VoidAction) {
        self.service = service
        self.assetData = assetData
        self.onComplete = onComplete
    }

    var title: String {
        Localized.Nft.Report.reportButtonTitle
    }

    var progressMessage: String {
        Localized.Common.loading
    }

    func submitReport(reason: String) {
        state = .loading
        Task {
            do {
                try await service.report(report: ReportNft(
                    collectionId: assetData.collection.id.identifier,
                    assetId: assetData.asset.id.identifier,
                    reason: reason,
                ).map())
                state = .data(true)
                onComplete?()
            } catch {
                debugLog("Report NFT error: \(error)")
                state = .error(error)
            }
        }
    }
}
