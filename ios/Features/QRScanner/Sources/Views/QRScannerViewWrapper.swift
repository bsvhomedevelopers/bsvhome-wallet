// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI
import VisionKit

@MainActor
final class ScannerHostViewController: UIViewController {
    private let scanner: DataScannerViewController

    var onDidAppear: (() -> Void)?

    init(scanner: DataScannerViewController) {
        self.scanner = scanner
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError()
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        addChild(scanner)
        scanner.view.frame = view.bounds
        scanner.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(scanner.view)
        scanner.didMove(toParent: self)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        onDidAppear?()
    }
}

@MainActor
struct QRScannerViewWrapper {
    private let mode: QRScannerMode
    private var scanResult: (String) -> Void
    private var dataScannerVC = DataScannerViewController(
        recognizedDataTypes: [.barcode(symbologies: [.qr])],
        qualityLevel: .balanced,
        recognizesMultipleItems: false,
        isHighFrameRateTrackingEnabled: true,
        isGuidanceEnabled: false,
        isHighlightingEnabled: false,
    )

    @Binding var isScannerReady: Bool

    init(mode: QRScannerMode = .single, isScannerReady: Binding<Bool>, scanResult: @escaping (String) -> Void) {
        self.mode = mode
        self.scanResult = scanResult
        _isScannerReady = isScannerReady
    }

    func startScanning() {
        guard !dataScannerVC.isScanning else { return }
        do {
            try dataScannerVC.startScanning()
            isScannerReady = true
        } catch {
            isScannerReady = false
        }
    }

    static func checkDeviceQRScanningSupport() throws(QRScannerError) {
        guard DataScannerViewController.isSupported else {
            throw QRScannerError.notSupported
        }

        guard DataScannerViewController.isAvailable else {
            throw QRScannerError.permissionsNotGranted
        }
    }
}

// MARK: - UIViewControllerRepresentable

extension QRScannerViewWrapper: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> ScannerHostViewController {
        dataScannerVC.delegate = context.coordinator
        let host = ScannerHostViewController(scanner: dataScannerVC)
        host.onDidAppear = startScanning
        return host
    }

    func updateUIViewController(_: ScannerHostViewController, context _: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(self, mode: mode)
    }

    static func dismantleUIViewController(_ controller: ScannerHostViewController, coordinator _: Coordinator) {
        controller.children.compactMap { $0 as? DataScannerViewController }.forEach { $0.stopScanning() }
    }
}

// MARK: - Coordinator

extension QRScannerViewWrapper {
    @MainActor
    class Coordinator {
        var parent: QRScannerViewWrapper
        private var delivery: QRScannerResultDelivery

        init(_ parent: QRScannerViewWrapper, mode: QRScannerMode) {
            self.parent = parent
            delivery = QRScannerResultDelivery(mode: mode)
        }

        func didAddItem(item: RecognizedItem) {
            guard case let .barcode(barcode) = item, let code = barcode.payloadStringValue else { return }
            guard delivery.accepts(code) else { return }
            parent.scanResult(code)
        }
    }
}

// MARK: - DataScannerViewControllerDelegate

extension QRScannerViewWrapper.Coordinator: DataScannerViewControllerDelegate {
    func dataScanner(_: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems _: [RecognizedItem]) {
        if let item = addedItems.first {
            didAddItem(item: item)
        }
    }

    func dataScanner(_: DataScannerViewController, didUpdate updatedItems: [RecognizedItem], allItems _: [RecognizedItem]) {
        if let item = updatedItems.first { didAddItem(item: item) }
    }
}
