import struct BsvHomeCore.BsvHomeBsvAddressRecord

extension BsvHomeBsvAddressRecord {
    var recordIdentifier: String {
        switch branch {
        case .receive: "receive-\(index)"
        case .change: "change-\(index)"
        }
    }
}
