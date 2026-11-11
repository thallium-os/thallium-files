import QtQuick

// A choice of a few labelled options, the chosen one filled and outlined.
Flow {
    id: chips
    property var options: []
    property string current: ""
    signal picked(string key)
    spacing: 6

    Repeater {
        model: chips.options
        delegate: Rectangle {
            id: chip
            required property var modelData
            readonly property bool chosen: chips.current === chip.modelData.key
            width: chipLabel.implicitWidth + 24
            height: 30
            radius: Theme.radiusSmall
            color: chip.chosen ? Theme.selected : (chipMouse.containsMouse ? Theme.surfaceHigh : Theme.surface)
            border.width: chip.chosen ? 1 : 0
            border.color: Theme.outline
            Text {
                id: chipLabel
                anchors.centerIn: parent
                text: chip.modelData.label
                color: chip.chosen ? Theme.ink : Theme.inkDim
                font.family: Theme.family
                font.pixelSize: 12
            }
            MouseArea {
                id: chipMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: chips.picked(chip.modelData.key)
            }
        }
    }
}
