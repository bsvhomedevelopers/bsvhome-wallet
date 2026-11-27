import BsvHomeCore

actor ServiceStatusServiceMock: BsvHomeServiceStatusProtocol {
    nonisolated let endpoints: [BsvHomeServiceEndpoint]
    private let statuses: [String: BsvHomeLatencyStatus]
    private(set) var requestedURLs: [String] = []

    init(endpoints: [BsvHomeServiceEndpoint], statuses: [String: BsvHomeLatencyStatus]) {
        self.endpoints = endpoints
        self.statuses = statuses
    }

    nonisolated func getEndpoints() -> [BsvHomeServiceEndpoint] {
        endpoints
    }

    func getEndpointStatus(url: String) async -> BsvHomeLatencyStatus {
        requestedURLs.append(url)
        return statuses[url] ?? .error
    }
}
