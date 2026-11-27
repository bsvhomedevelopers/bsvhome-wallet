// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.BsvHomeRewardsState
import struct BsvHomeCore.RedemptionResult
import protocol BsvHomeCore.BsvHomeRewardsServiceProtocol
import struct BsvHomeCore.Rewards
import Primitives

public extension BsvHomeRewardsServiceProtocol {
    func getRewards(wallet: Primitives.Wallet) async throws -> Rewards {
        try await getRewards(walletId: wallet.id.id)
    }

    func createReferral(wallet: Primitives.Wallet, code: String) async throws -> Rewards {
        try await createReferral(wallet: wallet.map(), code: code)
    }

    func useReferralCode(wallet: Primitives.Wallet, code: String) async throws {
        try await useReferralCode(wallet: wallet.map(), code: code)
    }

    func redeem(wallet: Primitives.Wallet, redemptionId: String) async throws -> RedemptionResult {
        try await redeem(wallet: wallet.map(), redemptionId: redemptionId)
    }

    func referralLink(code: String) throws -> URL {
        guard let url = URL(string: referralLink(code: code)) else {
            throw AnyError("invalid referral link")
        }
        return url
    }
}
