import QtQuick
import QtQuick.Controls
Column {
    id:root;objectName:"labels-"+account
    required property string account
    required property var navigation
    property bool disclosed:true
    property bool expanded:true
    readonly property var labels:Mail.labelSets[account]||[]
    width:parent.width;spacing:Theme.unit(3)
    onDisclosedChanged:if(disclosed)Mail.ensureLabels(account)
    Component.onCompleted:if(disclosed)Mail.ensureLabels(account)
    Row{width:parent.width
        FluidButton{width:parent.width-Theme.unit(70);glyph:"label";text:root.navigation.collapsed?"":"Labels";tip:"Expand labels";onClicked:root.expanded=!root.expanded}
        FluidButton{glyph:"refresh";tip:"Refresh labels";visible:!root.navigation.collapsed;onClicked:Mail.ensureLabels(root.account,true)}
        FluidButton{glyph:"add";tip:"Create label";visible:!root.navigation.collapsed;onClicked:root.navigation.labelActionRequested("create",root.account,{})}
    }
    Item{width:parent.width;height:Math.max(0,reveal.value);clip:true
        Motion{id:reveal;target:root.expanded?Math.min(root.labels.length,7)*Theme.unit(34):0;enabled:Theme.motion}
        ListView{id:list;anchors.fill:parent;model:root.labels;clip:true;reuseItems:true;cacheBuffer:Theme.unit(34);boundsBehavior:Flickable.StopAtBounds
            ScrollBar.vertical:ScrollBar{implicitWidth:3}
            delegate:Item{required property var modelData;objectName:"label-"+modelData.id;width:list.width;height:Theme.unit(34)
                Rectangle{anchors.fill:parent;anchors.margins:Theme.unit(2);radius:Theme.unit(10);color:Theme.selected;visible:Mail.account===root.account&&Mail.folder==="label:"+modelData.id}
                FluidButton{anchors.fill:parent;glyph:"label";text:root.navigation.collapsed?"":modelData.name;labelWidth:Math.max(0,width-Theme.unit(58));tip:modelData.name;tint:Mail.account===root.account&&Mail.folder==="label:"+modelData.id?Theme.accent:Theme.muted;onClicked:Mail.setView("label:"+modelData.id,root.account)}
                MouseArea{anchors.fill:parent;acceptedButtons:Qt.RightButton;onClicked:{context.label=modelData;context.popup()}}
            }
        }
    }
    FluidMenu{id:context;objectName:"labelsContext-"+root.account;property var label:({})
        onOpened:root.navigation.overlayOpen=true
        onClosed:root.navigation.overlayOpen=false
        MenuItem{objectName:"renameLabelAction-"+root.account;text:"Rename label…";onTriggered:root.navigation.labelActionRequested("rename",root.account,context.label)}
        MenuItem{objectName:"deleteLabelAction-"+root.account;text:"Delete label…";onTriggered:root.navigation.labelActionRequested("delete",root.account,context.label)}
    }
}
