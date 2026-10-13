import QtQuick

// A labelled button, as in the reference's dialog footers.
Rectangle {
    id: button
    property string text: ""
    property bool primary: false
    signal clicked

    implicitWidth: label.implicitWidth + 34
    implicitHeight: Theme.controlHeight
    radius: Theme.radiusSmall
    color: !button.enabled ? Theme.control
         : mouse.pressed ? Theme.outline
         : mouse.containsMouse || button.activeFocus ? Theme.controlHigh
         : button.primary ? Theme.controlHigh : Theme.control
    border.width: button.activeFocus ? 1 : 0
    border.color: Theme.outline
    opacity: button.enabled ? 1 : 0.45
    activeFocusOnTab: true

    Text {
        id: label
        anchors.centerIn: parent
        text: button.text
        color: Theme.ink
        font.family: Theme.family
        font.pixelSize: Theme.text
        font.weight: button.primary ? Font.Medium : Font.Normal
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: button.clicked()
    }

    Keys.onReturnPressed: button.clicked()
    Keys.onEnterPressed: button.clicked()
    Keys.onSpacePressed: button.clicked()
}
