import Components
import Localization
import Style
import SwiftUI

struct BsvColdWalletQRPlayer: View {
    private struct Playback: Hashable {
        let frames: [String]
        let playing: Bool
        let active: Bool
    }

    let frames: [String]
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.displayScale) private var displayScale
    @State private var index = 0
    @State private var playing = true
    @State private var image: UIImage?
    private let generator = QRCodeGenerator()

    var body: some View {
        VStack(spacing: Spacing.wowPay.coldWalletPage) {
            if let image {
                Image(uiImage: image).interpolation(.none).resizable()
                    .aspectRatio(1, contentMode: .fit)
                    .frame(maxWidth: .infinity)
                    .background(.white)
                    .accessibilityLabel(Localized.coldWalletQrFrames(index + 1, frames.count))
            } else if !frames.isEmpty {
                Text(Localized.coldWalletInvalidData).font(.wowPay.caption)
            }
            if !frames.isEmpty {
                Text(Localized.coldWalletQrFrames(index + 1, frames.count)).font(.wowPay.caption)
                    .accessibilityIdentifier("coldWalletQrFrameCount")
            }
            if frames.count > 1 {
                HStack {
                    Button { playing = false; index = (index + frames.count - 1) % frames.count } label: { Image(systemName: "chevron.left") }
                        .accessibilityLabel(Localized.coldWalletQrFrames((index + frames.count - 1) % frames.count + 1, frames.count))
                    Spacer()
                    Button(playing ? Localized.coldWalletQrPause : Localized.coldWalletQrPlay) { playing.toggle() }
                    Spacer()
                    Button { playing = false; index = (index + 1) % frames.count } label: { Image(systemName: "chevron.right") }
                        .accessibilityLabel(Localized.Common.continue)
                }
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
            }
        }
        .frame(maxWidth: .infinity).wowPayCard()
        .task(id: frames) { index = 0; playing = true; await render() }
        .task(id: index) { await render() }
        .task(id: Playback(frames: frames, playing: playing, active: scenePhase == .active)) {
            guard playing, scenePhase == .active, frames.count > 1 else { return }
            while !Task.isCancelled {
                do { try await Task.sleep(for: .milliseconds(700)) } catch { return }
                guard scenePhase == .active, playing, !frames.isEmpty else { return }
                index = (index + 1) % frames.count
            }
        }
    }

    private func render() async {
        guard frames.indices.contains(index) else { image = nil; return }
        let payload = frames[index]
        let pixels = Sizing.wowPay.coldWalletQR * displayScale
        let rendered = await generator.generate(from: payload, size: CGSize(width: pixels, height: pixels), correctionLevel: .medium)
        guard !Task.isCancelled, frames.indices.contains(index), frames[index] == payload else { return }
        image = rendered
    }
}
