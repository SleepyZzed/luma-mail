import QtQuick
Item {
    id:root;objectName:"authPanel";z:195
    readonly property var state:Mail.authState
    property bool opened:false
    visible:opened||position.value<height
    onStateChanged:{const phase=state.phase||"";if(phase==="starting")Qt.callLater(()=>cancelButton.forceActiveFocus());if(phase==="complete"){opened=true;completeDelay.restart()}else{completeDelay.stop();opened=phase!==""&&phase!=="cancelled"}}
    Motion{id:position;target:root.opened?(root.height-card.height)/2:root.height+Theme.unit(24);enabled:Theme.motion}
    Motion{id:breadth;target:Math.min(root.width-Theme.unit(48),Theme.unit(root.state.phase==="complete"?340:500));enabled:Theme.motion}
    Motion{id:depth;target:content.implicitHeight+Theme.unit(48);enabled:Theme.motion}
    Motion{id:corners;target:Theme.unit(root.state.phase==="complete"?27:24);enabled:Theme.motion}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent}}
    Rectangle{
        id:card;x:(root.width-width)/2;y:position.value;width:Math.max(0,breadth.value);height:Math.max(0,depth.value);radius:corners.value;clip:true;color:Theme.surface;border.width:1;border.color:Theme.outline
        Column{id:content;x:Theme.unit(24);y:Theme.unit(24);width:parent.width-2*x;spacing:Theme.unit(20)
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:root.state.phase==="complete"?"Connected":"Sign in with Google";color:Theme.text;font.pixelSize:Theme.unit(root.state.phase==="complete"?16:22);font.weight:Font.DemiBold}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:root.state.message||"";color:Theme.muted;font.pixelSize:Theme.unit(12)}
            Flow{width:parent.width;spacing:Theme.unit(10);visible:root.state.phase!=="complete"
                FluidButton{objectName:"authOpenBrowser";text:root.state.browserOpened?"Open browser again":"Open browser";glyph:"popout";primary:true;visible:root.state.phase==="waiting";onClicked:Mail.openAuthBrowser()}
                FluidButton{objectName:"authCopyLink";text:"Copy sign-in link";glyph:"link";visible:root.state.phase==="waiting";onClicked:Mail.copyAuthLink()}
                FluidButton{text:"Start again";primary:true;visible:root.state.phase==="error";onClicked:Mail.retryAuth()}
                FluidButton{id:cancelButton;objectName:"authCancel";text:root.state.phase==="error"?"Close":"Cancel sign-in";enabled:root.state.phase!=="finishing"&&root.state.phase!=="cancelling";onClicked:Mail.cancelAuth()}
            }
        }
    }
    Timer{id:completeDelay;interval:900;onTriggered:root.opened=false}
}
