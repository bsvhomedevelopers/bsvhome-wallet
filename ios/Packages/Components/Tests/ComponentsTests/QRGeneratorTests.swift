import Components
import CoreImage
import Testing
import UIKit

struct QRGeneratorTests {
    @Test
    func defaultCorrectionPreservesShortRequest() async throws {
        let request = "bitcoin:1BoatSLRHtKNngkdXEeobR76b53LETtpyT"
        let image = try #require(await QRCodeGenerator().generate(from: request, size: CGSize(width: 495, height: 495)))
        #expect(try decode(image) == request)
    }

    @Test
    func compactMultisigRequestRoundTripsAtDisplayResolution() async throws {
        let request = "WOWPAY-MSIG:1:" + String(repeating: "aZ19_-", count: 260)
        let generator = QRCodeGenerator()
        #expect(await generator.generate(from: request, size: CGSize(width: 495, height: 495)) == nil)
        let image = try #require(await generator.generate(from: request, size: CGSize(width: 495, height: 495), correctionLevel: .medium))
        #expect(try decode(image) == request)
    }

    @Test
    func excessivePayloadFailsWithoutTruncating() async {
        let request = "WOWPAY-MSIG:1:" + String(repeating: "aZ19_-", count: 600)
        #expect(await QRCodeGenerator().generate(from: request, size: CGSize(width: 495, height: 495), correctionLevel: .medium) == nil)
    }

    private func decode(_ image: UIImage) throws -> String? {
        let cgImage = try #require(image.cgImage)
        let detector = try #require(CIDetector(ofType: CIDetectorTypeQRCode, context: CIContext(options: [.useSoftwareRenderer: true]), options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]))
        return detector.features(in: CIImage(cgImage: cgImage))
            .compactMap { ($0 as? CIQRCodeFeature)?.messageString }
            .first
    }
}
