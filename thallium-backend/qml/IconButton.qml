import QtQuick
import QtQuick.Controls

// A square icon button: a dim inset with a hairline edge. Hover draws the
// edge and the glyph in the accent; `on` adds the selected wash, for toggles.
// `bare` drops the inset, for buttons that sit inside a row.
Rectangle {
    id: button
    property string icon: "file"
    property string hint: ""
    property bool on: false
    property bool bare: false
    property real iconSize: 16
    readonly property bool hovered: mouse.containsMouse
    signal clicked
    signal pressAndHold
    signal rightClicked

    implicitWidth: Theme.controlHeight
    implicitHeight: Theme.controlHeight
    radius: 0
    color: button.on ? Theme.cWash
         : button.bare ? (mouse.containsMouse ? Theme.cDim : "transparent")
         : Theme.cDim
    border.width: button.bare && !button.on ? 0 : 1
    border.color: button.on || (mouse.containsMouse && button.enabled) ? Theme.cGreen : Theme.cLine
    opacity: button.enabled ? 1 : 0.38

    Behavior on border.color { ColorAnimation { duration: Theme.tFast; easing.type: Easing.OutCubic } }

    Icon {
        anchors.centerIn: parent
        name: button.icon
        size: button.iconSize
        shade: 0.85
        tint: button.on || (mouse.containsMouse && button.enabled) ? Theme.cGreen : "transparent"
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        pressAndHoldInterval: 400
        cursorShape: Qt.PointingHandCursor
        onClicked: function (event) {
            if (event.button === Qt.RightButton)
                button.rightClicked()
            else
                button.clicked()
        }
        onPressAndHold: button.pressAndHold()
    }

    ToolTip.visible: button.hint.length > 0 && mouse.containsMouse
    ToolTip.delay: 600
    ToolTip.text: button.hint
}
