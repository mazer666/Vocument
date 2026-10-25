import AppKit
import SwiftUI
import VocumentFFI
import VocumentMacKit

/// Custom AppKit canvas view providing native macOS text editing, IME, keyboard shortcuts, and VoiceOver (`NSTextInputClient`, `ADR-0020`, `UI-009`).
@MainActor
final class VocumentNativeTextCanvasView: NSView, @preconcurrency NSTextInputClient {
    var snapshot: FfiEditorSessionSnapshot? {
        didSet {
            updateFrameSize()
            needsDisplay = true
        }
    }

    var onInsertText: ((String) -> Void)?
    var onInsertParagraphBreak: (() -> Void)?
    var onDeleteBackward: (() -> Void)?
    var onDeleteForward: (() -> Void)?
    var onHitTest: ((UInt32, Float, Float) -> Void)?
    var onToggleBold: (() -> Void)?
    var onToggleItalic: (() -> Void)?
    var onUndo: (() -> Void)?
    var onRedo: (() -> Void)?
    var onSave: (() -> Void)?

    private var markedString: NSMutableAttributedString = NSMutableAttributedString()
    private var markedSelectedRange: NSRange = NSRange(location: NSNotFound, length: 0)
    private var isCursorVisible: Bool = true
    private var blinkTimer: Timer?

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupCaretBlink()
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupCaretBlink()
        wantsLayer = true
    }

    override func viewWillMove(toWindow newWindow: NSWindow?) {
        super.viewWillMove(toWindow: newWindow)
        if newWindow == nil {
            blinkTimer?.invalidate()
            blinkTimer = nil
        } else if blinkTimer == nil {
            setupCaretBlink()
        }
    }

    private func setupCaretBlink() {
        blinkTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                self.isCursorVisible.toggle()
                self.needsDisplay = true
            }
        }
    }

    private func updateFrameSize() {
        guard let snap = snapshot, !snap.pages.isEmpty else { return }
        var totalHeight: CGFloat = 40.0
        var maxWidth: CGFloat = 800.0

        for page in snap.pages {
            totalHeight += CGFloat(page.heightPt) + 40.0
            if CGFloat(page.widthPt) > maxWidth {
                maxWidth = CGFloat(page.widthPt)
            }
        }

        let newSize = NSSize(width: max(maxWidth + 80.0, bounds.width), height: max(totalHeight, bounds.height))
        if frame.size != newSize {
            frame.size = newSize
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        guard let snap = snapshot else { return }
        guard let context = NSGraphicsContext.current?.cgContext else { return }

        // Background canvas fill
        NSColor.windowBackgroundColor.setFill()
        dirtyRect.fill()

        var currentY: CGFloat = 30.0

        for page in snap.pages {
            let pageOriginX = max(40.0, (bounds.width - CGFloat(page.widthPt)) / 2.0)
            let pageRect = NSRect(x: pageOriginX, y: currentY, width: CGFloat(page.widthPt), height: CGFloat(page.heightPt))

            // Draw paper drop shadow & page background
            context.saveGState()
            context.setShadow(offset: CGSize(width: 0, height: -3), blur: 8.0, color: NSColor.black.withAlphaComponent(0.12).cgColor)
            NSColor.white.setFill()
            let pagePath = NSBezierPath(roundedRect: pageRect, xRadius: 4.0, yRadius: 4.0)
            pagePath.fill()
            context.restoreGState()

            // Page border outline
            NSColor.separatorColor.setStroke()
            pagePath.lineWidth = 1.0
            pagePath.stroke()

            // Selection highlights on this page
            for sel in snap.selectionRects where sel.pageIndex == page.pageIndex {
                let selRect = NSRect(
                    x: pageOriginX + CGFloat(sel.rect.x),
                    y: currentY + CGFloat(sel.rect.y),
                    width: CGFloat(sel.rect.width),
                    height: CGFloat(sel.rect.height)
                )
                NSColor.selectedTextBackgroundColor.withAlphaComponent(0.35).setFill()
                selRect.fill()
            }

            // Render typeset text runs
            for line in page.lines {
                for run in line.runs {
                    var font = NSFont.systemFont(ofSize: CGFloat(run.fontSizePt))
                    var traits: NSFontDescriptor.SymbolicTraits = []
                    if run.isBold { traits.insert(.bold) }
                    if run.isItalic { traits.insert(.italic) }
                    if !traits.isEmpty {
                        let desc = font.fontDescriptor.withSymbolicTraits(traits)
                        font = NSFont(descriptor: desc, size: CGFloat(run.fontSizePt)) ?? font
                    }

                    let attrs: [NSAttributedString.Key: Any] = [
                        .font: font,
                        .foregroundColor: NSColor.labelColor,
                    ]

                    let runPoint = NSPoint(
                        x: pageOriginX + CGFloat(run.rect.x),
                        y: currentY + CGFloat(run.rect.y)
                    )
                    (run.text as NSString).draw(at: runPoint, withAttributes: attrs)
                }
            }

            // Draw Marked Text (inline IME composition preview with underlined style)
            if snap.cursorPageIndex == page.pageIndex, let cRect = snap.cursorRect, self.hasMarkedText() {
                let imeX = pageOriginX + CGFloat(cRect.x)
                let imeY = currentY + CGFloat(cRect.y)
                let imeFont = NSFont.systemFont(ofSize: max(12.0, CGFloat(cRect.height)))
                let imeAttrs: [NSAttributedString.Key: Any] = [
                    .font: imeFont,
                    .foregroundColor: NSColor.labelColor,
                    .underlineStyle: NSUnderlineStyle.single.rawValue,
                ]
                (self.markedString.string as NSString).draw(at: NSPoint(x: imeX, y: imeY), withAttributes: imeAttrs)
            }

            // Draw Caret on current cursor page
            if isCursorVisible, snap.cursorPageIndex == page.pageIndex, let cRect = snap.cursorRect {
                let caretOffset = self.hasMarkedText() ? CGFloat(self.markedString.length) * 8.0 : 0.0
                let caretX = pageOriginX + CGFloat(cRect.x) + caretOffset
                let caretY = currentY + CGFloat(cRect.y)
                let caretRect = NSRect(x: caretX, y: caretY, width: 2.0, height: max(14.0, CGFloat(cRect.height)))

                NSColor.controlAccentColor.setFill()
                caretRect.fill()
            }

            currentY += CGFloat(page.heightPt) + 40.0
        }
    }

    // MARK: - Mouse Interaction & Hit Testing

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let clickPoint = self.convert(event.locationInWindow, from: nil)

        guard let snap = snapshot else { return }
        var currentY: CGFloat = 30.0

        for page in snap.pages {
            let pageOriginX = max(40.0, (bounds.width - CGFloat(page.widthPt)) / 2.0)
            let pageRect = NSRect(x: pageOriginX, y: currentY, width: CGFloat(page.widthPt), height: CGFloat(page.heightPt))

            if pageRect.contains(clickPoint) {
                let localX = Float(clickPoint.x - pageOriginX)
                let localY = Float(clickPoint.y - currentY)
                self.onHitTest?(page.pageIndex, localX, localY)
                return
            }
            currentY += CGFloat(page.heightPt) + 40.0
        }
    }

    // MARK: - Keyboard & Shortcut Dispatch

    override func keyDown(with event: NSEvent) {
        // Keyboard shortcuts
        if event.modifierFlags.contains(.command) {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "b":
                self.onToggleBold?()
                return
            case "i":
                self.onToggleItalic?()
                return
            case "z":
                if event.modifierFlags.contains(.shift) {
                    self.onRedo?()
                } else {
                    self.onUndo?()
                }
                return
            case "s":
                self.onSave?()
                return
            default:
                break
            }
        }

        // Delegate to system input manager (IME, accent popups, standard typing)
        self.interpretKeyEvents([event])
    }

    override func insertNewline(_ sender: Any?) {
        self.onInsertParagraphBreak?()
    }

    override func deleteBackward(_ sender: Any?) {
        self.onDeleteBackward?()
    }

    override func deleteForward(_ sender: Any?) {
        self.onDeleteForward?()
    }

    // MARK: - NSTextInputClient Implementation (IME, VoiceOver, Accent Popups)

    func insertText(_ string: Any, replacementRange: NSRange) {
        let text: String
        if let str = string as? String {
            text = str
        } else if let attr = string as? NSAttributedString {
            text = attr.string
        } else {
            text = ""
        }

        markedString = NSMutableAttributedString()
        markedSelectedRange = NSRange(location: NSNotFound, length: 0)

        if replacementRange.location != NSNotFound && replacementRange.length > 0 {
            for _ in 0..<replacementRange.length {
                self.onDeleteBackward?()
            }
        }

        if !text.isEmpty {
            self.onInsertText?(text)
        }
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if replacementRange.location != NSNotFound && replacementRange.length > 0 {
            for _ in 0..<replacementRange.length {
                self.onDeleteBackward?()
            }
        }
        if let str = string as? String {
            markedString = NSMutableAttributedString(string: str)
        } else if let attr = string as? NSAttributedString {
            markedString = NSMutableAttributedString(attributedString: attr)
        }
        markedSelectedRange = selectedRange
        needsDisplay = true
    }

    func unmarkText() {
        markedString = NSMutableAttributedString()
        markedSelectedRange = NSRange(location: NSNotFound, length: 0)
        needsDisplay = true
    }

    func selectedRange() -> NSRange {
        guard let snap = snapshot else { return NSRange(location: 0, length: 0) }
        return NSRange(location: Int(snap.cursorOffset), length: 0)
    }

    func markedRange() -> NSRange {
        if markedString.length > 0 {
            return NSRange(location: 0, length: markedString.length)
        }
        return NSRange(location: NSNotFound, length: 0)
    }

    func hasMarkedText() -> Bool {
        markedString.length > 0
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard let snap = snapshot else { return nil }
        let text = snap.plainText as NSString
        guard range.location < text.length else { return nil }
        let validLength = min(range.length, text.length - range.location)
        let subRange = NSRange(location: range.location, length: validLength)
        actualRange?.pointee = subRange
        return NSAttributedString(string: text.substring(with: subRange))
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        [.font, .foregroundColor, .underlineStyle]
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let snap = snapshot, let cRect = snap.cursorRect else {
            return window?.convertToScreen(NSRect(origin: frame.origin, size: .zero)) ?? .zero
        }
        var currentY: CGFloat = 30.0
        var pageOriginX: CGFloat = 40.0
        for page in snap.pages {
            if page.pageIndex == snap.cursorPageIndex {
                pageOriginX = max(40.0, (bounds.width - CGFloat(page.widthPt)) / 2.0)
                break
            }
            currentY += CGFloat(page.heightPt) + 40.0
        }

        let localRect = NSRect(
            x: pageOriginX + CGFloat(cRect.x),
            y: currentY + CGFloat(cRect.y),
            width: max(2.0, CGFloat(cRect.width)),
            height: max(14.0, CGFloat(cRect.height))
        )
        let windowRect = self.convert(localRect, to: nil)
        return window?.convertToScreen(windowRect) ?? localRect
    }

    func characterIndex(for point: NSPoint) -> Int {
        guard let snap = snapshot else { return 0 }
        return Int(snap.cursorOffset)
    }

    // MARK: - VoiceOver Accessibility

    override func accessibilityRole() -> NSAccessibility.Role? {
        .textArea
    }

    override func accessibilityLabel() -> String? {
        "Vocument Dokumenteneditor"
    }

    override func accessibilityValue() -> Any? {
        snapshot?.plainText ?? ""
    }
}
