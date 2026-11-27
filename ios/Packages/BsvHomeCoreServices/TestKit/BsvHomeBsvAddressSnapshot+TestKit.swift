import struct BsvHomeCore.BsvHomeBsvAddressRecord
import struct BsvHomeCore.BsvHomeBsvAddressSnapshot
import enum BsvHomeCore.BsvHomeBsvDiscoveryState

public extension BsvHomeBsvAddressSnapshot {
    static func mock(
        records: [BsvHomeBsvAddressRecord]? = nil,
        currentAddress: String? = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA",
        receiveScanned: UInt32 = 0,
        changeScanned: UInt32 = 0,
        allocationPaused: Bool = false,
        receiveCount: UInt32? = nil,
        selectedIndex: UInt32? = nil,
        canAdd: Bool = true,
        discovery: BsvHomeBsvDiscoveryState = .notStarted,
    ) -> BsvHomeBsvAddressSnapshot {
        let records = records ?? currentAddress.map {
            [BsvHomeBsvAddressRecord(branch: .receive, index: 0, address: $0, status: .unused, isCurrent: true, isMain: true)]
        } ?? []
        return BsvHomeBsvAddressSnapshot(
            records: records,
            currentAddress: currentAddress,
            receiveScanned: receiveScanned,
            changeScanned: changeScanned,
            discovery: discovery,
            allocationPaused: allocationPaused,
            receiveCount: receiveCount ?? records.filter { $0.branch == .receive }.map { $0.index + 1 }.max() ?? 0,
            selectedIndex: selectedIndex ?? records.first { $0.branch == .receive && $0.address == currentAddress }?.index,
            canAdd: canAdd,
        )
    }
}
