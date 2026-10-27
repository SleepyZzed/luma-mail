import QtQuick
import QtQuick.Controls
ApplicationWindow {
    id:window
    width:1440;height:900;minimumWidth:760;minimumHeight:560;visible:true;title:"Luma Mail";color:Theme.window
    flags:isWindows?Qt.Window:Qt.Window|Qt.FramelessWindowHint
    font.family:Theme.family
    onClosing:(close)=>{if((Mail.settings.close_to_tray===undefined||Mail.settings.close_to_tray)&&trayAvailable){close.accepted=false;window.hide()}else{close.accepted=false;Mail.quit()}}
    Item {
        id:scene;anchors.fill:parent
        readonly property real railGoal:Theme.unit(rail.collapsed?76:240)
        readonly property real inboxGoal:Mail.readerOpen?(width<Theme.unit(1000)?0:Math.min(Theme.unit(450),(width-railGoal)*.4)):width-railGoal
        readonly property real readerGoalX:railGoal+inboxGoal
        readonly property real readerGoalWidth:Mail.readerOpen?width-readerGoalX:reader.lastLayoutWidth
        function projectionSpec(){return reader.projectionSpec(readerGoalX,readerGoalWidth)}
        Motion{id:railMotion;target:scene.railGoal;enabled:Theme.motion}
        Motion{id:inboxMotion;target:scene.inboxGoal;enabled:Theme.motion}
        Sidebar {
            id:rail;width:railMotion.value;height:parent.height
            onComposeAt:(x,y,w,h)=>{composer.inlineDock=null;composer.parent=scene;composer.inlineMode=false;composer.origin=Qt.rect(x,y,w,h);Mail.requestCompose("new")}
            onSettingsRequested:settings.opened=true
        }
        Inbox {
            id:inbox;scene:scene;x:rail.width;height:parent.height-Theme.unit(44)
            width:Math.max(0,inboxMotion.value)
            onContextRequested:(row,x,y)=>{context.row=row;context.x=x;context.y=y;context.open()}
        }
        Reader {
            id:reader;x:inbox.x+inbox.width;width:Math.max(0,scene.width-x);height:parent.height-Theme.unit(44);layoutWidth:scene.readerGoalWidth;projected:morph.active;inlineComposer:composer;visible:width>1
            onReplyAt:(mode,message)=>{if(composer.shown&&composer.inlineMode){reader.dockInline(composer);return}composer.inlineMode=true;reader.dockInline(composer);Mail.requestCompose(mode,message)}
        }
        ActivityPill{x:rail.width+Theme.unit(28);anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(13)}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(20);anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(9);text:"Undo";glyph:"undo";visible:Mail.canUndo;onClicked:Mail.send("undo")}
        RowMorph{id:morph}
        Composer{id:composer;scene:scene;shown:false;onDismissRequested:composer.close()}
        SettingsSheet{id:settings;anchors.fill:parent;onCloseRequested:opened=false}
        Rectangle {
            id:errorToast;anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(58);width:Math.min(Theme.unit(600),parent.width-Theme.unit(64));height:message.implicitHeight+Theme.unit(30);radius:Theme.unit(16);color:Theme.raised;border.width:1;border.color:Theme.accent;visible:Mail.error!=="";z:200
            Text{id:message;x:Theme.unit(16);y:Theme.unit(15);width:parent.width-Theme.unit(65);text:Mail.error;wrapMode:Text.Wrap;color:Theme.text;font.pixelSize:Theme.unit(12)}
            FluidButton{anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;glyph:"close";onClicked:Mail.clearError()}
        }
        Menu {
            id:context;property var row:({})
            MenuItem{text:"Archive";onTriggered:Mail.mutate("archive",context.row)}
            MenuItem{text:"Move to Trash";onTriggered:Mail.mutate("trash",context.row)}
            MenuItem{text:context.row.unread?"Mark read":"Mark unread";onTriggered:Mail.mutate(context.row.unread?"read":"unread",context.row)}
            MenuItem{text:context.row.starred?"Remove star":"Star";onTriggered:Mail.mutate(context.row.starred?"unstar":"star",context.row)}
            MenuItem{text:context.row.important?"Mark unimportant":"Mark important";onTriggered:Mail.mutate(context.row.important?"unimportant":"important",context.row)}
            MenuItem{text:"Move to Inbox";visible:Mail.folder!=="inbox";height:visible?implicitHeight:0;onTriggered:Mail.mutate("inbox",context.row)}
            MenuSeparator{}
            MenuItem{text:"Select all loaded";onTriggered:Mail.selectAll()}
        }
    }
    Connections {
        target:Mail
        function onProjectRow(row,x,y,w,h){morph.place(row,Qt.rect(x,y,w,h),scene.projectionSpec(),true)}
        function onReturnRow(row){const target=inbox.projectionTarget(row.key,scene.width-scene.railGoal);if(target)morph.place(row,target,scene.projectionSpec(),false)}
        function onReaderChanged(){if(morph.active&&morph.opening&&Mail.readerOpen)morph.destination=scene.projectionSpec()}
        function onComposeRequested(mode){composer.open(mode)}
    }
    Shortcut{sequence:"Ctrl+N";onActivated:{composer.inlineDock=null;composer.inlineMode=false;composer.parent=scene;Mail.requestCompose("new")}}
    Shortcut{sequence:"Escape";onActivated:{if(settings.opened)settings.opened=false;else if(composer.shown)composer.close();else if(Mail.selecting)Mail.clearSelection();else Mail.closeReader()}}
    Shortcut{sequence:"Ctrl+R";onActivated:Mail.setView(Mail.folder,Mail.account,Mail.search)}
}
