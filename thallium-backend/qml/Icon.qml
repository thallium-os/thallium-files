import QtQuick
import QtQuick.Effects

// A Lucide outline icon from icons/, recoloured to the theme's ink and
// shaded by opacity, or drawn in `tint` where it needs a colour of its own
// (the accent, or the ink of a file tile). The SVGs are white, so they
// would vanish on a light Thallium variant if drawn as shipped.
Item {
    id: icon
    property string name: "file"
    property real size: 18
    property real shade: 0.62
    property color tint: "transparent"
    readonly property bool tinted: icon.tint.a > 0

    width: size
    height: size

    Image {
        id: image
        anchors.fill: parent
        sourceSize: Qt.size(icon.size * 2, icon.size * 2)
        source: "icons/" + icon.name + ".svg"
        visible: false
        smooth: true
    }

    MultiEffect {
        anchors.fill: parent
        source: image
        // Shades were tuned for light ink on dark; dark ink on a light
        // variant needs more of itself to read.
        opacity: icon.tinted ? 1 : Theme.dark ? icon.shade : Math.min(1, icon.shade * 1.6)
        brightness: 0.0
        colorization: 1.0
        colorizationColor: icon.tinted ? icon.tint : Theme.ink
    }
}
