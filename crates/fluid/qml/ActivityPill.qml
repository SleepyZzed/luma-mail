import QtQuick
Item {
    id:root
    property string label:Mail.activity
    property bool busy:Mail.activityKind!=="idle"&&Mail.activityKind!=="error"
    implicitWidth:breadth.value;implicitHeight:Theme.unit(30)
    Motion{id:breadth;target:Math.min(Theme.unit(350),caption.implicitWidth+Theme.unit(root.busy?56:34));enabled:Theme.motion}
    Rectangle {anchors.fill:parent;radius:height/2;color:Theme.surface;border.width:1;border.color:Theme.outline}
    Rectangle {id:dot;x:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter;width:Math.max(1,presence.value);height:width;radius:height/2;color:root.busy?Theme.accent:Theme.muted
        Motion{id:presence;target:Theme.unit(root.busy?8:5);enabled:Theme.motion}
    }
    Text {id:caption;anchors.left:dot.right;anchors.leftMargin:Theme.unit(9);anchors.right:parent.right;anchors.rightMargin:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter;text:root.label;elide:Text.ElideRight;color:Theme.muted;font.family:Theme.family;font.pixelSize:Theme.unit(10)}
}
