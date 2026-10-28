import QtQuick
import QtQuick.Controls.Basic
import QtMultimedia
Item {
    id:root;objectName:"attachmentPane";z:150
    property bool shown:false
    property bool revealed:false
    property bool closing:false
    function clock(ms){const seconds=Math.max(0,Math.floor(ms/1000));return Math.floor(seconds/60)+":"+(seconds%60).toString().padStart(2,"0")}
    readonly property bool moving:horizontal.running||vertical.running||breadth.running||extent.running
    readonly property rect origin:AttachmentViewer.origin.width>0?AttachmentViewer.origin:Qt.rect(width/2,height/2,30,30)
    readonly property real goalWidth:Math.min(width-Theme.unit(48),Theme.unit(960))
    readonly property real goalHeight:Math.min(height-Theme.unit(64),Theme.unit(740))
    visible:shown
    function close(){if(!shown||closing)return;AttachmentViewer.pause();closing=true;revealed=false;Qt.callLater(finishClose)}
    function finishClose(){if(closing&&!moving){shown=false;closing=false;AttachmentViewer.close()}}
    Connections{target:AttachmentViewer;function onChanged(){if(AttachmentViewer.opened){if(!root.shown){horizontal.snap(root.origin.x);vertical.snap(root.origin.y);breadth.snap(root.origin.width);extent.snap(root.origin.height);root.shown=true;root.forceActiveFocus()}root.closing=false;root.revealed=true}else if(root.shown&&!root.closing)root.close()}}
    Motion{id:horizontal;target:root.revealed?(root.width-root.goalWidth)/2:root.origin.x;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:vertical;target:root.revealed?(root.height-root.goalHeight)/2:root.origin.y;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:breadth;target:root.revealed?root.goalWidth:root.origin.width;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:extent;target:root.revealed?root.goalHeight:root.origin.height;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:shade;target:root.revealed?.45:0;epsilon:.002;enabled:Theme.motion}
    Rectangle{anchors.fill:parent;color:"#000";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:root.close()}}
    Rectangle{
        id:card;x:horizontal.value;y:vertical.value;width:Math.max(1,breadth.value);height:Math.max(1,extent.value);radius:Theme.unit(23);color:Theme.surface;border.width:1;border.color:Theme.outline;clip:true
        Text{textFormat:Text.PlainText;x:Theme.unit(25);y:Theme.unit(23);width:parent.width-Theme.unit(120);text:AttachmentViewer.name;elide:Text.ElideMiddle;color:Theme.text;font.pixelSize:Theme.unit(16);font.weight:Font.DemiBold}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(16);y:Theme.unit(15);glyph:"close";tip:"Close preview";onClicked:root.close()}
        Item{id:body;x:Theme.unit(24);y:Theme.unit(77);width:Math.max(0,parent.width-2*x);height:Math.max(0,parent.height-y-Theme.unit(78))
            Image{anchors.centerIn:parent;width:sourceSize.width>0?Math.min(parent.width,sourceSize.width*Theme.scale):parent.width;height:sourceSize.height>0?Math.min(parent.height,sourceSize.height*Theme.scale):parent.height;source:AttachmentViewer.image;cache:false;asynchronous:true;fillMode:Image.PreserveAspectFit;visible:AttachmentViewer.kind==="Image"}
            ScrollView{anchors.fill:parent;visible:AttachmentViewer.kind==="Text"||AttachmentViewer.kind==="Word document";clip:true
                TextArea{text:AttachmentViewer.text;textFormat:TextEdit.PlainText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;color:Theme.text;font.family:Theme.family;font.pixelSize:Theme.unit(13);background:Item{}}
            }
            VideoOutput{id:video;anchors.fill:parent;visible:AttachmentViewer.kind==="Video";fillMode:VideoOutput.PreserveAspectFit;Component.onCompleted:AttachmentViewer.setVideoSink(videoSink);Component.onDestruction:AttachmentViewer.clearVideoSink(videoSink)}
            Text{textFormat:Text.PlainText;anchors.centerIn:parent;text:AttachmentViewer.kind==="Audio"?"Audio preview":"";visible:text!==""&&!AttachmentViewer.loading;color:Theme.muted;font.pixelSize:Theme.unit(22)}
            Column{anchors.centerIn:parent;width:Math.min(parent.width,Theme.unit(360));spacing:Theme.unit(14);visible:AttachmentViewer.loading
                Text{textFormat:Text.PlainText;text:"Preparing preview…";color:Theme.muted;font.pixelSize:Theme.unit(13)}
                ProgressBar{width:parent.width;from:0;to:1;value:AttachmentViewer.progress;indeterminate:AttachmentViewer.progress<=0}
            }
            Rectangle{anchors.fill:parent;color:Theme.surface;visible:AttachmentViewer.error!=="";Text{textFormat:Text.PlainText;anchors.centerIn:parent;width:Math.min(parent.width,Theme.unit(540));text:AttachmentViewer.error;wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(13)}}
        }
        Row{x:Theme.unit(24);y:parent.height-Theme.unit(61);spacing:Theme.unit(12)
            FluidButton{text:"Save to folder…";glyph:"download";onClicked:AttachmentViewer.saveCurrent()}
            FluidButton{text:AttachmentViewer.playing?"Pause":"Play";visible:AttachmentViewer.kind==="Audio"||AttachmentViewer.kind==="Video";enabled:!AttachmentViewer.loading&&AttachmentViewer.error==="";onClicked:AttachmentViewer.playPause()}
            Slider{width:Math.max(Theme.unit(80),card.width-Theme.unit(570));from:0;to:Math.max(1,AttachmentViewer.duration);value:AttachmentViewer.position;visible:AttachmentViewer.kind==="Audio"||AttachmentViewer.kind==="Video";onMoved:AttachmentViewer.seek(value)}
            Text{textFormat:Text.PlainText;anchors.verticalCenter:parent.verticalCenter;text:root.clock(AttachmentViewer.position)+" / "+root.clock(AttachmentViewer.duration);color:Theme.muted;font.pixelSize:Theme.unit(10);visible:AttachmentViewer.kind==="Audio"||AttachmentViewer.kind==="Video"}
            Slider{width:Theme.unit(80);from:0;to:1;value:AttachmentViewer.volume;visible:AttachmentViewer.kind==="Audio"||AttachmentViewer.kind==="Video";Accessible.name:"Volume";onMoved:AttachmentViewer.volume=value}
        }
    }
    Shortcut{sequence:"Space";enabled:root.shown&&(AttachmentViewer.kind==="Audio"||AttachmentViewer.kind==="Video");onActivated:AttachmentViewer.playPause()}
}
