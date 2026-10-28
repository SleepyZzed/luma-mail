import QtQuick
Item {
    id:root
    property real initial:240
    property real minimum:160
    property real maximum:480
    property real liveValue:initial
    property real startValue:initial
    property var pendingValue:undefined
    readonly property bool dragging:drag.active
    readonly property real value:drag.active?liveValue:pendingValue===undefined?initial:pendingValue
    onInitialChanged:if(pendingValue===initial)pendingValue=undefined
    function commit(value){pendingValue=Math.max(minimum,Math.min(maximum,value));committed(pendingValue)}
    signal committed(real value)
    width:Theme.unit(10);activeFocusOnTab:true
    Accessible.name:"Resize pane"
    HoverHandler{id:hover;cursorShape:Qt.SplitHCursor}
    Rectangle{anchors.horizontalCenter:parent.horizontalCenter;anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(3);height:Math.min(Theme.unit(68),parent.height);radius:width/2;color:Theme.accent;opacity:Math.max(0,Math.min(1,reveal.value));Motion{id:reveal;target:(hover.hovered||root.dragging||root.activeFocus)?0.8:0;enabled:Theme.motion;epsilon:.002}}
    DragHandler{id:drag;target:null;acceptedButtons:Qt.LeftButton;xAxis.enabled:true;yAxis.enabled:false
        onActiveChanged:if(active){root.startValue=root.pendingValue===undefined?root.initial:root.pendingValue;root.liveValue=root.startValue}else root.commit(root.liveValue)
        onTranslationChanged:if(active)root.liveValue=Math.max(root.minimum,Math.min(root.maximum,root.startValue+translation.x/Theme.scale))
    }
    Keys.onPressed:event=>{if(event.key===Qt.Key_Left||event.key===Qt.Key_Right){root.commit(root.value+(event.key===Qt.Key_Left?-10:10));event.accepted=true}}
    Connections{target:Mail;function onErrorChanged(){if(Mail.error)root.pendingValue=undefined}}
}
