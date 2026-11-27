import Foundation
import BsvHomeCore
import Primitives
@testable import Settings
import Store
import StoreTestKit
import Testing

@MainActor
struct DeveloperEntryProducerTests {
    @Test
    func wowPayReadsOnlyTheActualDeviceAndStoreMetadata() async {
        let service = SettingsDeveloperServiceMock(noHandle: .init())
        let model = makeModel(service: service, environment: .wowPay)
        await model.load()
        #expect(service.calls == ["device", "store"])
        #expect(model.deviceId == "public-device-fixture")
        #expect(model.platformStore == "local")
        #expect(model.deviceToken.isEmpty)
    }

    @Test
    func retainedProductionMetadataLoadingKeepsItsOriginalOwner() async {
        let service = SettingsDeveloperServiceMock(noHandle: .init())
        let model = makeModel(service: service, environment: .production)
        await model.load()
        #expect(service.calls == ["device", "push"])
    }

    @Test
    func resetDelegatesOnlyToTheExistingPendingTransactionOwner() async {
        let service = SettingsDeveloperServiceMock(noHandle: .init())
        let model = makeModel(service: service, environment: .wowPay)
        await model.resetPendingTransactions()
        #expect(service.calls == ["clearPending"])
        #expect(!model.isResettingPending)
    }

    private func makeModel(service: SettingsDeveloperServiceMock, environment: BsvHomeRuntimeEnvironment) -> DeveloperViewModel {
        let db = DB.mock()
        return DeveloperViewModel(walletId: .view(chain: .bitcoinSv, address: "1BitcoinEaterAddressDontSendf59kuE"), service: service, transactionStore: TransactionStore(db: db), assetStore: AssetStore(db: db), stakeStore: StakeStore(db: db), bannerStore: BannerStore(db: db), priceStore: PriceStore(db: db), environment: environment)
    }
}
