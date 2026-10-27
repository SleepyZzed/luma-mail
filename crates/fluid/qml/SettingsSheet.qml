import QtQuick
import QtQuick.Controls
Item {
    id:root
    property bool opened:false
    signal closeRequested()
    visible:opened||panel.y<height;z:100
    Motion{id:backdrop;target:root.opened?.45:0;epsilon:.002;enabled:Theme.motion}
    Motion{id:position;target:root.opened?(root.height-panel.height)/2:root.height+Theme.unit(30);enabled:Theme.motion}
    Rectangle{anchors.fill:parent;color:"#000";opacity:Math.max(0,Math.min(.5,backdrop.value)); MouseArea{anchors.fill:parent;onClicked:root.closeRequested()}}
    Rectangle {
        id:panel;width:Math.min(parent.width-Theme.unit(48),Theme.unit(620));height:Math.min(parent.height-Theme.unit(48),Theme.unit(700));x:(parent.width-width)/2;y:position.value;radius:Theme.unit(25);color:Theme.surface;border.width:1;border.color:Theme.outline
        Text {textFormat:Text.PlainText;x:Theme.unit(30);y:Theme.unit(30);text:"Make yourself at home";font.pixelSize:Theme.unit(23);font.weight:Font.DemiBold;color:Theme.text}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(18);y:Theme.unit(24);glyph:"close";tip:"Close settings";onClicked:root.closeRequested()}
        Column{x:Theme.unit(30);y:Theme.unit(93);width:parent.width-2*x;spacing:Theme.unit(24)
            Text {textFormat:Text.PlainText;text:"ACCOUNTS";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Repeater{model:Mail.settings.accounts||[];delegate:Row{required property var modelData;spacing:Theme.unit(14);Avatar{sender:modelData.name||modelData.email}Column{spacing:5;Text {textFormat:Text.PlainText;text:modelData.name||modelData.email;color:Theme.text;font.pixelSize:Theme.unit(13)}Text {textFormat:Text.PlainText;text:modelData.email;color:Theme.muted;font.pixelSize:Theme.unit(11)}}}}
            FluidButton {text:"Sign in with Google";glyph:"add";primary:true;onClicked:Mail.send("authStart",{drive:true,settings:true,photos:true})}
            Rectangle{width:parent.width;height:1;color:Theme.outline}
            Text {textFormat:Text.PlainText;text:"YOUR INTERFACE";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Row{width:parent.width;spacing:Theme.unit(20);Text {textFormat:Text.PlainText;text:"UI scale";color:Theme.text;font.pixelSize:Theme.unit(13);anchors.verticalCenter:parent.verticalCenter}Slider{from:75;to:200;stepSize:5;value:Mail.settings.ui_scale_percent||100;width:Theme.unit(250);onMoved:Mail.send("settingsUpdate",{ui_scale_percent:Math.round(value)})}Text {textFormat:Text.PlainText;text:Math.round(Mail.settings.ui_scale_percent||100)+"%";color:Theme.muted;anchors.verticalCenter:parent.verticalCenter}}
            CheckBox{text:"Reduce motion";checked:Mail.settings.reduced_motion||false;onToggled:Mail.send("settingsUpdate",{reduced_motion:checked})}
            Text {textFormat:Text.PlainText;text:"STORAGE & PRIVACY";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Text {textFormat:Text.PlainText;width:parent.width;text:"Mail stays in Gmail. Only the configured, bounded cache is retained. No mailbox mirror or local draft store.";wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
            Row{spacing:Theme.unit(12);ComboBox{model:["Cache off","50 MiB","100 MiB","250 MiB","500 MiB"];currentIndex:[0,50,100,250,500].indexOf(Mail.settings.cache_mib||0);onActivated:Mail.send("settingsUpdate",{cache_mib:[0,50,100,250,500][currentIndex]})}FluidButton{text:"Clear cache";onClicked:Mail.send("cacheClear")}}
        }
    }
}
