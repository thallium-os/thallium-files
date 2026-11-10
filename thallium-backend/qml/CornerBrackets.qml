import QtQuick

// Small L-shaped accent marks in the corners of a panel. The corner with
// the chamfer is left out by default: the cut already marks it.
Item {
  id: brackets
  property int size: 11
  property real weight: 1.5
  property color color: Theme.cGreen
  property bool topRight: false
  property bool bottomLeft: true
  property bool bottomRight: true

  Repeater {
    model: [
      { x: 0, y: 0, h: 1, v: 1, on: true },
      { x: 1, y: 0, h: -1, v: 1, on: brackets.topRight },
      { x: 0, y: 1, h: 1, v: -1, on: brackets.bottomLeft },
      { x: 1, y: 1, h: -1, v: -1, on: brackets.bottomRight }
    ]
    delegate: Item {
      required property var modelData
      visible: modelData.on
      x: modelData.x === 0 ? 0 : brackets.width - brackets.size
      y: modelData.y === 0 ? 0 : brackets.height - brackets.size
      width: brackets.size
      height: brackets.size
      Rectangle {
        width: brackets.size
        height: brackets.weight
        x: 0
        y: modelData.v > 0 ? 0 : brackets.size - height
        color: brackets.color
      }
      Rectangle {
        width: brackets.weight
        height: brackets.size
        x: modelData.h > 0 ? 0 : brackets.size - width
        y: 0
        color: brackets.color
      }
    }
  }
}
