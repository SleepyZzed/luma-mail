import QtQuick
Item {
    id:root
    property bool busy:true
    property bool animate:true
    property string label:"Opening private view…"
    property bool beat:false
    implicitHeight:Theme.unit(112)
    Motion{id:presence;target:root.busy?1:0;epsilon:.002;enabled:Theme.motion}
    visible:presence.value>.002
    Timer{interval:620;repeat:true;running:root.busy&&root.visible&&root.animate&&Theme.motion;onTriggered:root.beat=!root.beat}
    Rectangle{
        width:Math.min(parent.width,Theme.unit(260));height:Theme.unit(38)*Math.max(0,Math.min(1,presence.value));radius:height/2
        color:Theme.surface;border.color:Theme.outline;border.width:1;clip:true
        Row{x:Theme.unit(14);anchors.verticalCenter:parent.verticalCenter;spacing:Theme.unit(5)
            Repeater{model:3;Rectangle{required property int index;width:Theme.unit(4);height:width;radius:width/2;color:Theme.accent
                y:(1-dot.value)*Theme.unit(4);Motion{id:dot;target:root.busy?(root.beat?(index===1?1:.2):(index===1?.2:1)):0;epsilon:.005;stiffness:180;damping:24;enabled:Theme.motion&&root.animate}
            }}
        }
        Text{textFormat:Text.PlainText;x:Theme.unit(42);anchors.verticalCenter:parent.verticalCenter;width:parent.width-x-Theme.unit(12);text:root.label;elide:Text.ElideRight;color:Theme.muted;font.pixelSize:Theme.unit(11)}
    }
    Column{y:Theme.unit(56);width:parent.width;spacing:Theme.unit(9);scale:Math.max(0,Math.min(1,presence.value));transformOrigin:Item.TopLeft
        Rectangle{width:parent.width*.76;height:Theme.unit(5);radius:height/2;color:Theme.outline;opacity:.45}
        Rectangle{width:parent.width*.52;height:Theme.unit(5);radius:height/2;color:Theme.outline;opacity:.3}
    }
}
