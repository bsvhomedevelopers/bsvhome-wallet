// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import class BsvHomeCore.Config
import enum BsvHomeCore.BsvHomeSlippageCheck
import protocol BsvHomeCore.BsvHomeSwapQuoteServiceProtocol
import BsvHomeCorePrimitives
import InfoSheet
import Localization
import Primitives
import PrimitivesComponents
import Validators

@MainActor
@Observable
public final class SwapSlippageViewModel {
    private static let maxFractionDigits: Int = 2
    private nonisolated static let formatter = NumericFormatter()

    private let service: any BsvHomeSwapQuoteServiceProtocol
    private let onSelect: (SwapSlippage) -> Void
    private let suggestionsBps: [UInt32]
    private let maxPercent: Double

    let placeholder: String
    var isAuto: Bool
    var inputModel: InputValidationViewModel
    var infoSheet: InfoSheetType?

    public init(service: any BsvHomeSwapQuoteServiceProtocol, chain: Chain, slippage: SwapSlippage, onSelect: @escaping (SwapSlippage) -> Void) {
        self.service = service
        self.onSelect = onSelect
        let config = Config.shared.swapConfig()
        suggestionsBps = config.slippageSuggestionsBps
        maxPercent = Double(config.maxSlippageBps) / 100
        placeholder = Self.format(bps: service.defaultSlippage(chain: chain.rawValue).bps)
        let input: String
        switch slippage {
        case .auto:
            isAuto = true
            input = ""
        case let .manual(value):
            isAuto = false
            input = Self.format(bps: value)
        }
        inputModel = InputValidationViewModel(
            mode: .onDemand,
            validators: [
                SwapSlippageValidator(
                    service: service,
                    minimumDisplay: PercentFormatter.unsigned.string(Double(config.minSlippageBps) / 100),
                    maximumDisplay: PercentFormatter.unsigned.string(Double(config.maxSlippageBps) / 100),
                ),
            ],
        )
        inputModel.text = input
    }

    var title: String {
        Localized.Swap.slippage
    }

    var autoTitle: String {
        Localized.Swap.slippageAuto
    }

    var autoDescription: String {
        Localized.Swap.slippageAutoDescription
    }

    var selectedBps: UInt32 {
        Self.bps(from: inputModel.text) ?? 0
    }

    var errorText: String? {
        inputModel.error?.localizedDescription
    }

    var isConfirmEnabled: Bool {
        isAuto || (inputModel.isValid && selectedBps > 0)
    }

    var warningText: String? {
        guard inputModel.isValid, service.slippageCheck(bps: selectedBps) == .high else { return nil }
        return Localized.Swap.slippageWarning
    }

    var suggestions: [SlippageSuggestion] {
        suggestionsBps.map { SlippageSuggestion(bps: $0, percentText: Self.format(bps: $0)) }
    }

    func onSelect(suggestion: SlippageSuggestion) {
        inputModel.text = suggestion.inputValue
    }

    func onSelectInfo() {
        infoSheet = .slippage
    }

    func sanitize(_ text: String) -> String {
        NumberSanitizer(
            maximumFractionDigits: Self.maxFractionDigits,
            maximumIntegerDigits: String(Int(maxPercent)).count,
        ).sanitize(text)
    }

    func confirm() {
        onSelect(isAuto ? .auto : .manual(bps: selectedBps))
    }

    nonisolated static func bps(from text: String) -> UInt32? {
        guard let percent = formatter.double(from: text), percent > 0 else { return nil }
        return UInt32((percent * 100).rounded())
    }

    private static func format(bps: UInt32) -> String {
        (Double(bps) / 100).formatted(.number.precision(.fractionLength(0 ... 2)))
    }
}

private struct SwapSlippageValidator: TextValidator {
    private let service: any BsvHomeSwapQuoteServiceProtocol
    private let minimumDisplay: String
    private let maximumDisplay: String

    init(service: any BsvHomeSwapQuoteServiceProtocol, minimumDisplay: String, maximumDisplay: String) {
        self.service = service
        self.minimumDisplay = minimumDisplay
        self.maximumDisplay = maximumDisplay
    }

    func validate(_ text: String) throws {
        guard let bps = SwapSlippageViewModel.bps(from: text) else { return }
        switch service.slippageCheck(bps: bps) {
        case .valid, .high: return
        case .belowMinimum: throw AnyError(Localized.Common.minimumValue(minimumDisplay))
        case .aboveMaximum: throw AnyError(Localized.Common.maximumValue(maximumDisplay))
        }
    }

    var id: String {
        "SwapSlippageValidator"
    }
}
