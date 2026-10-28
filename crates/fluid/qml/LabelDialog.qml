import QtQuick
import QtQuick.Controls.Basic
Item {
    id:root;objectName:"labelDialog";z:170;visible:opened||position.value<height
    property bool opened:false
    property bool pending:false
    property string operation:"create"
    property string account:""
    property var label:({})
    function edit(operation,account,label){root.operation=operation;root.account=account;root.label=label;name.text=label.name||"";pending=false;opened=true;if(operation!=="delete")Qt.callLater(()=>name.forceActiveFocus())}
    function submit(){if(pending)return;pending=true;Mail.send("labels",{account:account,operation:operation,label:label.id||"",name:name.text,confirmed:operation==="delete"})}
    Motion{id:position;target:root.opened?(root.height-card.height)/2:root.height+Theme.unit(20);enabled:Theme.motion}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:if(!root.pending)root.opened=false}}
    Rectangle{id:card;x:(parent.width-width)/2;y:position.value;width:Math.min(parent.width-Theme.unit(48),Theme.unit(490));height:content.implicitHeight+Theme.unit(48);radius:Theme.unit(22);color:Theme.surface;border.width:1;border.color:Theme.outline
        Column{id:content;x:Theme.unit(24);y:Theme.unit(24);width:parent.width-2*x;spacing:Theme.unit(18)
            Text{textFormat:Text.PlainText;text:root.operation==="delete"?"Delete label?":root.operation==="rename"?"Rename label":"Create label";color:Theme.text;font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;visible:root.operation==="delete";text:"Remove “"+(root.label.name||"")+"” from Gmail? Messages carrying this label are kept.";color:Theme.muted;font.pixelSize:Theme.unit(12)}
            FluidField{id:name;objectName:"labelName";width:parent.width;visible:root.operation!=="delete";enabled:!root.pending;selectByMouse:true;maximumLength:225;placeholderText:"Label name";color:Theme.text;font.pixelSize:Theme.unit(13);padding:Theme.unit(12);background:Rectangle{radius:Theme.unit(10);color:Theme.window;border.width:1;border.color:name.activeFocus?Theme.accent:Theme.outline}onAccepted:if(text.trim())root.submit()}
            Row{spacing:Theme.unit(12)
                FluidButton{text:"Cancel";enabled:!root.pending;onClicked:root.opened=false}
                FluidButton{text:root.pending?"Saving…":root.operation==="delete"?"Delete label":"Save";primary:true;enabled:!root.pending&&(root.operation==="delete"||name.text.trim()!=="");onClicked:root.submit()}
            }
        }
    }
    Connections{target:Mail
        function onReceived(kind,data){if(root.pending&&kind==="labels"&&data.account===root.account&&data.operation===root.operation){root.pending=false;root.opened=false}}
        function onErrorChanged(){if(Mail.error)root.pending=false}
    }
}
