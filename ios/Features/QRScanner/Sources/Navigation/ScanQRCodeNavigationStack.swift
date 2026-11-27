// Copyright (c). Gem Wallet. All rights reserved.

import Localization
import Primitives
import SwiftUI

public struct ScanQRCodeNavigationStack: View {
    @Environment(\.dismiss) private var dismiss

    private let resources = QRScanResources()
    private let scanType: QRScanType
    private let mode: QRScannerMode
    private let hint: String?
    private let allowsPhotoLibrary: Bool

    let action: (String) -> Void

    public init(scanType: QRScanType, mode: QRScannerMode = .single, hint: String? = nil, allowsPhotoLibrary: Bool = true, action: @escaping (String) -> Void) {
        self.scanType = scanType
        self.mode = mode
        self.hint = hint
        self.allowsPhotoLibrary = allowsPhotoLibrary
        self.action = action
    }

    public var body: some View {
        NavigationStack {
            QRScannerScene(resources: resources, scanType: scanType, mode: mode, hint: hint, allowsPhotoLibrary: allowsPhotoLibrary, action: action)
                .navigationTitle(Localized.Wallet.scan)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button(resources.dismissText) {
                            dismiss()
                        }
                    }
                }
        }
    }
}
