import QtQuick
Item {
    id:root;objectName:"confirmationSheet";z:175;visible:opened||position.value<height
    property bool opened:false
    property string heading:""
    property string message:""
    property string buttonText:""
    property string command:""
    property var parameters:({})
    function ask(heading,message,buttonText,command,parameters){root.heading=heading;root.message=message;root.buttonText=buttonText;root.command=command;root.parameters=parameters;root.opened=true;cancel.forceActiveFocus()}
    function submit(){if(!opened)return;if(Mail.send(command,parameters))opened=false}
    Motion{id:position;target:root.opened?(root.height-card.height)/2:root.height+Theme.unit(20);enabled:Theme.motion}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:root.opened=false}}
    Rectangle{id:card;x:(parent.width-width)/2;y:position.value;width:Math.min(parent.width-Theme.unit(48),Theme.unit(520));height:content.implicitHeight+Theme.unit(48);radius:Theme.unit(22);color:Theme.surface;border.width:1;border.color:Theme.outline
        Column{id:content;x:Theme.unit(24);y:Theme.unit(24);width:parent.width-2*x;spacing:Theme.unit(20)
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:root.heading;color:Theme.text;font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:root.message;color:Theme.muted;font.pixelSize:Theme.unit(12)}
            Row{spacing:Theme.unit(12);FluidButton{id:cancel;text:"Cancel";onClicked:root.opened=false}FluidButton{text:root.buttonText;primary:true;onClicked:root.submit()}}
        }
    }
}
