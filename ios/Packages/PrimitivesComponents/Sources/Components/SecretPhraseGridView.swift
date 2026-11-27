// Copyright (c). Gem Wallet. All rights reserved.

import Style
import SwiftUI

public struct SecretPhraseGridView: View {
    public enum Style {
        case standard
        case wowPay
    }

    private let rows: [[WordIndex]]
    private let highlightIndex: Int?
    private let style: Style

    public init(
        rows: [[WordIndex]],
        highlightIndex: Int? = .none,
        style: Style = .standard,
    ) {
        self.rows = rows
        self.highlightIndex = highlightIndex
        self.style = style
    }

    public var body: some View {
        Grid(alignment: .leading) {
            ForEach(rows, id: \.self) { words in
                GridRow {
                    ForEach(words) { word in
                        HStack {
                            Text("\(word.index + 1).")
                                .fontWeight(.semibold)
                                .foregroundStyle(style == .wowPay ? Colors.WowPay.secondaryText : Colors.grayLight)
                                .multilineTextAlignment(.leading)
                                .padding(.leading, style == .wowPay ? .zero : .small)
                            Text(word.word)
                                .fontWeight(.semibold)
                                .foregroundStyle(style == .wowPay ? Colors.WowPay.heading : Colors.black)
                                .allowsTightening(false)
                                .fixedSize(horizontal: false, vertical: style == .wowPay)
                                .multilineTextAlignment(.leading)
                                .accessibilityIdentifier("word_\(word.index)")
                            Spacer()
                        }
                        .padding(.small)
                        .font(style == .wowPay ? .wowPay.label : nil)
                        .background(style == .wowPay ? Colors.WowPay.pale : Colors.listStyleColor)
                        .cornerRadius(cornerRadius)
                        .overlay {
                            if highlightIndex == word.index {
                                RoundedRectangle(cornerRadius: cornerRadius)
                                    .stroke(style == .wowPay ? Colors.WowPay.brand : Colors.blue, lineWidth: .space2)
                            }
                        }
                    }
                }.padding(.vertical, .space2)
            }
        }
        .padding(.horizontal, style == .wowPay ? .zero : .medium)
        .frame(maxWidth: .scene.content.maxWidth)
    }

    private var cornerRadius: CGFloat {
        style == .wowPay ? Sizing.wowPay.inputCorner : .space10
    }
}
