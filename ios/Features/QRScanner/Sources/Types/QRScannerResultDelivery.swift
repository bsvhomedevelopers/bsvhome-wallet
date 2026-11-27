struct QRScannerResultDelivery {
    let mode: QRScannerMode
    private var lastValue: String?

    init(mode: QRScannerMode) {
        self.mode = mode
    }

    mutating func accepts(_ value: String) -> Bool {
        guard !value.isEmpty, value != lastValue else { return false }
        if mode == .single, lastValue != nil { return false }
        lastValue = value
        return true
    }
}
