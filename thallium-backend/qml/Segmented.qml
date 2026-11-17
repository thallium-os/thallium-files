import QtQuick

// Icon options side by side on one control, the chosen one filled.
Rectangle {
    id: segmented
    property var options: []
    property string current: ""
    signal picked(string key)

    implicitWidth: row.implicitWidth + 8
    implicitHeight: Theme.controlHeight
    radius: Theme.radius
    color: Theme.control

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: segmented.options
            delegate: Rectangle {
                id: option
                required property var modelData
                readonly property bool chosen: segmented.current === option.modelData.key
                width: 30
                height: segmented.height - 8
                radius: Theme.radiusSmall
                color: option.chosen ? Theme.controlHigh : (optionMouse.containsMouse ? Theme.surfaceHigh : "transparent")

                Icon {
                    anchors.centerIn: parent
                    name: option.modelData.icon
                    size: 16
                    shade: option.chosen ? 1 : 0.55
                }

                MouseArea {
                    id: optionMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: segmented.picked(option.modelData.key)
                }
            }
        }
    }
}
