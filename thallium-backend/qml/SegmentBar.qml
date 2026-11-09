import QtQuick

// A row of sheared segments, lit from the left: `lit` of `count` in the
// accent, the rest dead. `share` (0..1) sets `lit` when given.
Item {
    id: bar
    property int count: 20
    property real share: -1
    property int lit: bar.share >= 0 ? Math.round(Math.max(0, Math.min(1, bar.share)) * bar.count) : 0
    property color color: Theme.cGreen
    property color dead: Theme.cDead
    property int gap: 3

    implicitHeight: 6
    implicitWidth: 160

    Row {
        anchors.fill: parent
        spacing: bar.gap
        Repeater {
            model: bar.count
            delegate: Rectangle {
                required property int index
                width: (bar.width - bar.gap * (bar.count - 1)) / bar.count
                height: bar.height
                color: index < bar.lit ? bar.color : bar.dead
                antialiasing: true
                transform: Matrix4x4 {
                    // Shear about the segment's own baseline.
                    matrix: Qt.matrix4x4(1, Theme.shear, 0, -Theme.shear * bar.height,
                                         0, 1, 0, 0,
                                         0, 0, 1, 0,
                                         0, 0, 0, 1)
                }
            }
        }
    }
}
