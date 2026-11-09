import QtQuick
import QtQuick.Shapes

// A raised surface with its top-right corner cut, and the cut drawn in the
// accent: the Thallium 81 panel, as the Store draws it. Children sit above
// the shape.
Item {
  id: panel
  property color fill: Theme.cPanel
  property color stroke: Theme.cLine
  property bool accentEdge: true
  property int cut: Theme.chamfer

  Shape {
    anchors.fill: parent
    preferredRendererType: Shape.CurveRenderer
    ShapePath {
      fillColor: panel.fill
      strokeColor: panel.stroke
      strokeWidth: 1
      joinStyle: ShapePath.MiterJoin
      startX: 0.5; startY: 0.5
      PathLine { x: panel.width - panel.cut; y: 0.5 }
      PathLine { x: panel.width - 0.5; y: panel.cut }
      PathLine { x: panel.width - 0.5; y: panel.height - 0.5 }
      PathLine { x: 0.5; y: panel.height - 0.5 }
      PathLine { x: 0.5; y: 0.5 }
    }
    ShapePath {
      fillColor: "transparent"
      strokeColor: panel.accentEdge ? Theme.cGreen : "transparent"
      strokeWidth: 2
      capStyle: ShapePath.FlatCap
      startX: panel.width - panel.cut; startY: 0.5
      PathLine { x: panel.width - 0.5; y: panel.cut }
    }
  }
}
