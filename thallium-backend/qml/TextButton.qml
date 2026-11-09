import QtQuick

// A labelled button. The primary action is filled with the accent and
// lettered in the void colour; the others are a raised panel with a
// hairline edge that turns to the accent under the pointer.
Rectangle {
  id: button
  property string text: ""
  property bool primary: false
  // Destructive buttons arm on the first press and act on a second within
  // three seconds, as the Store's do.
  property bool confirm: false
  property bool armed: false
  property string armedText: "Click again"
  signal clicked

  Timer { id: disarm; interval: 3000; onTriggered: button.armed = false }

  implicitWidth: label.implicitWidth + 32
  implicitHeight: button.primary ? Theme.buttonHeight : Theme.controlHeight
  radius: 0
  color: button.armed ? Theme.cRed
    : button.primary ? (mouse.pressed ? Qt.darker(Theme.cGreen, 1.18)
              : mouse.containsMouse ? Qt.lighter(Theme.cGreen, 1.08) : Theme.cGreen)
    : mouse.containsMouse || button.activeFocus ? Theme.cDim : Theme.cPanel
  border.width: 1
  border.color: button.armed ? Theme.cRed
        : button.primary ? Theme.cGreen
        : mouse.containsMouse || button.activeFocus ? Theme.cGreen : Theme.cLine
  opacity: button.enabled ? 1 : 0.45
  activeFocusOnTab: true

  Behavior on color { ColorAnimation { duration: Theme.tFast } }

  function act() {
    if (button.confirm && !button.armed) {
      button.armed = true
      disarm.restart()
      return
    }
    button.armed = false
    disarm.stop()
    button.clicked()
  }

  Text {
    id: label
    anchors.centerIn: parent
    text: button.armed ? button.armedText : button.text
    color: button.primary || button.armed ? Theme.cBase : Theme.cFg
    font.family: Theme.fontHuman
    font.pixelSize: 13
    font.weight: button.primary || button.armed ? Font.Bold : Font.Medium
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: button.act()
  }

  Keys.onReturnPressed: button.act()
  Keys.onEnterPressed: button.act()
  Keys.onSpacePressed: button.act()
}
