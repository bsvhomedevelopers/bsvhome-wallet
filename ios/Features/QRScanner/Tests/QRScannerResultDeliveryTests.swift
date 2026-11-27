@testable import QRScanner
import Testing

struct QRScannerResultDeliveryTests {
    @Test
    func singleScanCannotDeliverAgainBeforeDismissal() {
        var delivery = QRScannerResultDelivery(mode: .single)
        let accepted = ["", "address", "address", "another-address"].map { delivery.accepts($0) }
        #expect(accepted == [false, true, false, false])
    }

    @Test
    func continuousScanAcceptsChangingAndCyclingFrames() {
        var delivery = QRScannerResultDelivery(mode: .continuous)
        let accepted = ["frame-1", "frame-1", "frame-2", "frame-1"].map { delivery.accepts($0) }
        #expect(accepted == [true, false, true, true])
    }
}
