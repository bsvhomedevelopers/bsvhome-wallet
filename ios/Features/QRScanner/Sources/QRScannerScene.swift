// Copyright (c). Gem Wallet. All rights reserved.

import Components
import PhotosUI
import Primitives
import Style
import SwiftUI

public struct QRScannerScene: View {
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss

    @State private var model: QRScannerSceneViewModel

    private let action: (String) -> Void
    private let mode: QRScannerMode
    private let hint: String?
    private let allowsPhotoLibrary: Bool

    public init(resources: QRScanResources, scanType: QRScanType, mode: QRScannerMode = .single, hint: String? = nil, allowsPhotoLibrary: Bool = true, action: @escaping (String) -> Void) {
        self.action = action
        self.mode = mode
        self.hint = hint
        self.allowsPhotoLibrary = allowsPhotoLibrary
        _model = State(initialValue: QRScannerSceneViewModel(resources: resources, scanType: scanType))
    }

    public var body: some View {
        ZStack {
            switch model.scannerState {
            case .scanning:
                QRScannerDisplayView(
                    configuration: model.overlayConfig,
                    hint: hint ?? model.hint,
                    mode: mode,
                    isScannerReady: $model.isScannerReady,
                    scanResult: onScan,
                )
                .ignoresSafeArea()
            case let .failure(error):
                let errorModel = QRScannerErrorViewModel(error: error)
                VStack {
                    Spacer()
                    StateEmptyView(
                        title: errorModel.title,
                        description: errorModel.description,
                        image: errorModel.image,
                    )
                    Spacer()
                    VStack(spacing: .medium) {
                        errorActionButtons(for: error)
                    }
                    .frame(maxWidth: .scene.button.maxWidth)
                    .padding()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .toolbar {
            if allowsPhotoLibrary {
                ToolbarItem(placement: .primaryAction) {
                    let imageName = model.resources.gallerySystemImage
                    photosPicker {
                        Image(systemName: imageName)
                            .bold()
                    }
                }
            }
        }
        .toolbarBackground(.visible, for: .navigationBar)
        .toast(message: $model.isPresentingToastMessage, offsetY: -model.toastOffset)
        .onChange(
            of: model.isScannerReady,
            initial: true,
            model.onChangeScannerReadyStatus,
        )
        .onChange(of: model.selectedPhoto, onLoadPhoto)
    }

    @ViewBuilder
    private func errorActionButtons(for error: QRScannerError) -> some View {
        switch error {
        case .notSupported:
            if allowsPhotoLibrary { photoLibraryButton.buttonStyle(.blue(paddingVertical: .zero)) }
        case .permissionsNotGranted:
            Button(action: onSelectOpenSettings) {
                actionLabel(model.resources.openSettings)
            }
            .buttonStyle(.blue(paddingVertical: .zero))
            if allowsPhotoLibrary { photoLibraryButton.buttonStyle(.amount(paddingHorizontal: .button.paddingHorizontal, paddingVertical: .zero, cornerRadius: Sizing.space12)) }
        }
    }

    private var photoLibraryButton: some View {
        let text = model.resources.selectFromPhotos
        return photosPicker {
            actionLabel(text)
        }
    }

    private func actionLabel(_ title: String) -> some View {
        Text(title)
            .frame(height: .scene.button.height)
    }

    private func photosPicker(
        @ViewBuilder label: @Sendable @escaping () -> some View,
    ) -> some View {
        PhotosPicker(
            selection: $model.selectedPhoto,
            matching: .images,
            photoLibrary: .shared(),
            label: label,
        )
    }
}

// MARK: - Actions

extension QRScannerScene {
    private func onSelectOpenSettings() {
        guard let settingsURL = URL(string: UIApplication.openSettingsURLString) else { return }
        openURL(settingsURL)
    }

    private func onLoadPhoto(_: PhotosPickerItem?, _ newValue: PhotosPickerItem?) {
        guard let newValue else { return }
        model.selectedPhoto = nil
        Task {
            if let code = await model.retrieveQRCode(photoItem: newValue) {
                onScan(code)
            } else {
                model.showDecodingError()
            }
        }
    }

    private func onScan(_ code: String) {
        action(code)
        if mode == .single { dismiss() }
    }
}
