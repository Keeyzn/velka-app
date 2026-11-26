// Fabrique les PNG d'un .iconset à la façon macOS (utilisé par tools/mac/paquet.sh).
//   swift tools/mac/icone.swift <icône carrée> <dossier .iconset>
// macOS attend une icône qui occupe environ 80 % du carré (marge transparente autour) : sans cette marge, l'icône
// paraît plus grosse que celles des autres applications dans le Dock et le Launchpad.
import AppKit

let args = CommandLine.arguments
guard args.count == 3, let src = NSImage(contentsOfFile: args[1]) else {
    FileHandle.standardError.write(Data("usage : icone.swift <png> <dossier>\n".utf8))
    exit(2)
}
let share = 0.805 // 824 px sur 1024, la grille d'icône d'Apple

func render(_ size: Int, to path: String) throws {
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ), let ctx = NSGraphicsContext(bitmapImageRep: rep) else { exit(1) }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = ctx
    ctx.imageInterpolation = .high
    let art = (Double(size) * share).rounded()
    let off = (Double(size) - art) / 2
    src.draw(in: NSRect(x: off, y: off, width: art, height: art), from: .zero, operation: .sourceOver, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    guard let png = rep.representation(using: .png, properties: [:]) else { exit(1) }
    try png.write(to: URL(fileURLWithPath: path))
}

for s in [16, 32, 128, 256] {
    try render(s, to: "\(args[2])/icon_\(s)x\(s).png")
    try render(s * 2, to: "\(args[2])/icon_\(s)x\(s)@2x.png")
}
try render(512, to: "\(args[2])/icon_512x512.png")
