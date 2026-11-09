import QtQuick

// The file-type mark: DIR, CODE, DOC or TXT in mono capitals, each in its
// own type colour.
Text {
    id: tag
    property string kind: "TXT"
    property real size: 9.5
    readonly property color tone: Theme.typeColor(tag.kind)

    text: tag.kind
    color: tag.tone
    font.family: Theme.fontMono
    font.pixelSize: Math.round(tag.size)
    font.bold: true
    font.letterSpacing: 1.4
}
