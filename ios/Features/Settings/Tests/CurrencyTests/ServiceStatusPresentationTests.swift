import BsvHomeCore
import Localization
@testable import Settings
import Testing

@MainActor
struct ServiceStatusPresentationTests {
    @Test(arguments: [BsvHomeServiceEndpointType.bitcoinSvRpc, .defiLlama, .api, .bsvHomeNode])
    func titlesDescribeTheActualEndpointProvider(type: BsvHomeServiceEndpointType) {
        let endpoint = BsvHomeServiceEndpoint(endpointType: type, host: "status.example.invalid", url: "https://status.example.invalid", flag: "🇺🇸")
        let model = ServiceStatusItemViewModel(endpoint: endpoint, status: .loading)
        let expected: String = switch type {
        case .bitcoinSvRpc: "WhatsOnChain"
        case .defiLlama: "DefiLlama"
        case .api: "API"
        case .bsvHomeNode: Localized.Nodes.bsvHomeWalletNode
        }
        #expect(model.title == expected + " 🇺🇸")
        #expect(model.id == endpoint.url)
        #expect(model.subtitle == endpoint.host)
        #expect(model.titleTag == "")
    }

    @Test
    func loadingAndTypedFailureStayAttachedToTheCorrectEndpointAcrossRefresh() async {
        let rpc = BsvHomeServiceEndpoint(endpointType: .bitcoinSvRpc, host: "api.whatsonchain.com", url: "https://api.whatsonchain.com/v1/bsv/main/chain/info", flag: "")
        let prices = BsvHomeServiceEndpoint(endpointType: .defiLlama, host: "coins.llama.fi", url: "https://coins.llama.fi/prices/current/coingecko:bitcoin-cash-sv", flag: "")
        let service = ServiceStatusServiceMock(endpoints: [rpc, prices], statuses: [rpc.url: .error, prices.url: .result(latency: Latency(latencyType: .fast, value: 12))])
        let model = ServiceStatusViewModel(service: service)
        #expect(model.itemModels.allSatisfy { $0.titleTag == "" })
        #expect(model.itemModels.map(\.title) == ["WhatsOnChain", "DefiLlama"])
        await model.load()
        #expect(model.itemModels.map(\.id) == [rpc.url, prices.url])
        #expect(model.itemModels[0].titleTag == Localized.Errors.error)
        #expect(model.itemModels[1].titleTag?.isEmpty == false)
        #expect(model.itemModels[1].titleTag != Localized.Errors.error)
        #expect(await service.requestedURLs.sorted() == [rpc.url, prices.url].sorted())
        await model.load()
        #expect(await service.requestedURLs.count == 4)
        #expect(model.itemModels[0].titleTag == Localized.Errors.error)
    }
}
