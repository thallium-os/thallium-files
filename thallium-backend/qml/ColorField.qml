import QtQuick
import QtQuick.Controls

// One editable colour: its name, a swatch that opens a picker, and its hex
// code, which can be typed or pasted.
Item {
    id: field

    property string label: ""
    property string value: "#000000"
    signal edited(string value)

    implicitWidth: 320
    implicitHeight: Theme.controlHeight

    // Hue, saturation and value while picking; read from `value` on open.
    property real hue: 0
    property real saturation: 0
    property real brightness: 0

    function pick() {
        field.edited(Qt.hsva(field.hue, field.saturation, field.brightness, 1).toString())
    }

    Text {
        id: name
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 100
        text: field.label
        color: Theme.inkSoft
        font.family: Theme.family
        font.pixelSize: Theme.text
    }

    Rectangle {
        id: swatch
        anchors.left: name.right
        anchors.verticalCenter: parent.verticalCenter
        width: 44
        height: 28
        radius: Theme.radiusSmall
        color: field.value
        border.width: 1
        border.color: swatchMouse.containsMouse || picker.opened ? Theme.outline : Theme.outlineSoft
        MouseArea {
            id: swatchMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                const c = Qt.lighter(field.value, 1.0)
                field.hue = Math.max(0, c.hsvHue)
                field.saturation = c.hsvSaturation
                field.brightness = c.hsvValue
                picker.open()
            }
        }
    }

    Rectangle {
        anchors.left: swatch.right
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 104
        height: 28
        radius: Theme.radiusSmall
        color: Theme.inset
        border.width: 1
        border.color: hex.activeFocus ? Theme.outline : Theme.outlineSoft
        TextInput {
            id: hex
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            verticalAlignment: TextInput.AlignVCenter
            text: field.value
            color: Theme.ink
            selectionColor: Theme.accentWash
            selectedTextColor: Theme.ink
            font.family: Theme.mono
            font.pixelSize: 12
            maximumLength: 7
            validator: RegularExpressionValidator { regularExpression: /#?[0-9a-fA-F]{0,6}/ }
            onTextEdited: {
                const code = hex.text.startsWith("#") ? hex.text : "#" + hex.text
                if (Theme.isColor(code))
                    field.edited(code.toLowerCase())
            }
            onEditingFinished: hex.text = field.value
        }
    }

    Popup {
        id: picker
        focus: true
        y: field.height + 4
        x: swatch.x
        padding: 12
        background: Rectangle {
            color: Theme.surfaceHigh
            radius: Theme.radius
            border.width: 1
            border.color: Theme.outlineSoft
        }

        contentItem: Row {
            spacing: 10

            // Saturation runs left to right, brightness bottom to top.
            Item {
                width: 180
                height: 140
                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    color: Qt.hsva(field.hue, 1, 1, 1)
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: "#ffffffff" }
                        GradientStop { position: 1; color: "#00ffffff" }
                    }
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    gradient: Gradient {
                        GradientStop { position: 0; color: "#00000000" }
                        GradientStop { position: 1; color: "#ff000000" }
                    }
                }
                Rectangle {
                    x: field.saturation * parent.width - width / 2
                    y: (1 - field.brightness) * parent.height - height / 2
                    width: 12
                    height: 12
                    radius: 6
                    color: "transparent"
                    border.width: 2
                    border.color: field.brightness > 0.5 ? "#000000" : "#ffffff"
                }
                MouseArea {
                    anchors.fill: parent
                    preventStealing: true
                    function place(mouse) {
                        field.saturation = Math.max(0, Math.min(1, mouse.x / width))
                        field.brightness = 1 - Math.max(0, Math.min(1, mouse.y / height))
                        field.pick()
                    }
                    onPressed: function (mouse) { place(mouse) }
                    onPositionChanged: function (mouse) { place(mouse) }
                }
            }

            Item {
                width: 16
                height: 140
                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    gradient: Gradient {
                        GradientStop { position: 0.000; color: "#ff0000" }
                        GradientStop { position: 0.167; color: "#ffff00" }
                        GradientStop { position: 0.333; color: "#00ff00" }
                        GradientStop { position: 0.500; color: "#00ffff" }
                        GradientStop { position: 0.667; color: "#0000ff" }
                        GradientStop { position: 0.833; color: "#ff00ff" }
                        GradientStop { position: 1.000; color: "#ff0000" }
                    }
                }
                Rectangle {
                    x: -2
                    y: field.hue * parent.height - height / 2
                    width: parent.width + 4
                    height: 4
                    radius: 2
                    color: "transparent"
                    border.width: 1
                    border.color: Theme.ink
                }
                MouseArea {
                    anchors.fill: parent
                    preventStealing: true
                    function place(mouse) {
                        field.hue = Math.max(0, Math.min(0.999, mouse.y / height))
                        field.pick()
                    }
                    onPressed: function (mouse) { place(mouse) }
                    onPositionChanged: function (mouse) { place(mouse) }
                }
            }
        }
    }
}
