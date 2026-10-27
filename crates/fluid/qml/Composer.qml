import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"composer"
    property bool shown:false
    property bool expanded:false
    property bool inlineMode:false
    property string phase:"editing"
    property Item scene
    property rect origin:Qt.rect(22,99,200,47)
    property Item inlineDock
    property bool manualPosition:false
    property point floatingPosition:Qt.point(0,0)
    property point dragStart:Qt.point(0,0)
    property bool formLoading:false
    readonly property real compactWidth:Math.min(Theme.unit(650),scene.width-Theme.unit(48))
    readonly property real compactHeight:Math.min(Theme.unit(535),scene.height-Theme.unit(48))
    readonly property real inlineHeight:Theme.unit(420)
    readonly property real editorWidth:inlineMode&&parent?parent.width:expanded?scene.width-Theme.unit(40):compactWidth
    readonly property real editorHeight:inlineMode?inlineHeight:expanded?scene.height-Theme.unit(40):compactHeight
    readonly property bool statusMode:phase==="sent"||phase==="sending"
    readonly property real goalWidth:statusMode?Math.min(editorWidth,Theme.unit(330)):editorWidth
    readonly property real goalHeight:inlineMode&&!shown?0:statusMode?Theme.unit(54):editorHeight
    readonly property real goalX:inlineMode?0:statusMode?scene.width-goalWidth-Theme.unit(24):expanded?Theme.unit(20):manualPosition?Math.max(Theme.unit(12),Math.min(scene.width-goalWidth-Theme.unit(12),floatingPosition.x)):scene.width-goalWidth-Theme.unit(24)
    readonly property real goalY:inlineMode?0:!shown?scene.height+Theme.unit(24):statusMode?scene.height-goalHeight-Theme.unit(24):expanded?Theme.unit(20):manualPosition?Math.max(Theme.unit(12),Math.min(scene.height-goalHeight-Theme.unit(12),floatingPosition.y)):scene.height-goalHeight-Theme.unit(24)
    readonly property bool moving:horizontal.running||vertical.running||breadth.running||depth.running
    signal dismissRequested()
    visible:shown||moving
    clip:true;z:70
    x:horizontal.value;y:vertical.value;width:Math.max(0,breadth.value);height:Math.max(0,depth.value)
    Motion{id:horizontal;target:root.goalX;enabled:Theme.motion&&!drag.active}
    Motion{id:vertical;target:root.goalY;enabled:Theme.motion&&!drag.active}
    Motion{id:breadth;target:root.goalWidth;enabled:Theme.motion}
    Motion{id:depth;target:root.goalHeight;enabled:Theme.motion}
    Rectangle {anchors.fill:parent;radius:Theme.unit(root.inlineMode?18:22);color:Theme.raised;border.width:1;border.color:Theme.outline}
    Item {
        id:canvas;width:root.editorWidth;height:root.editorHeight
    Item {
        id:header;width:root.width;height:Theme.unit(55)
        Text{x:Theme.unit(22);anchors.verticalCenter:parent.verticalCenter;text:root.phase==="sending"?"Sending your message…":root.phase==="sent"?"Sent. A little less on your mind.":root.inlineMode?"Reply":"New message";color:Theme.text;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold}
        Row {anchors.right:parent.right;anchors.rightMargin:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter;visible:root.phase==="editing"
            FluidButton{glyph:root.expanded?"minimize":"maximize";tip:root.expanded?"Restore composer":"Expand composer";onClicked:root.toggleExpanded()}
            FluidButton{glyph:"close";tip:"Close composer";onClicked:root.dismissRequested()}
        }
        DragHandler {
            id:drag;target:null;enabled:!root.inlineMode&&!root.expanded&&!root.statusMode;acceptedButtons:Qt.LeftButton
            onActiveChanged:if(active){root.dragStart=Qt.point(root.x,root.y);root.floatingPosition=root.dragStart;root.manualPosition=true}
            onTranslationChanged:if(active)root.floatingPosition=Qt.point(root.dragStart.x+translation.x,root.dragStart.y+translation.y)
        }
    }
    Column {
        id:fields;x:Theme.unit(22);y:header.height;width:parent.width-2*x;spacing:0;visible:root.phase==="editing"
        Row {width:parent.width;height:Theme.unit(39)
            Text{width:Theme.unit(38);anchors.verticalCenter:parent.verticalCenter;text:"To";font.pixelSize:Theme.unit(11);color:Theme.faint}
            TextField{id:to;objectName:"composeTo";width:parent.width-Theme.unit(108);height:parent.height;color:Theme.text;placeholderText:"Recipients";placeholderTextColor:Theme.faint;selectByMouse:true;font.pixelSize:Theme.unit(12);background:Item{} KeyNavigation.tab:subject;onTextEdited:root.sync()}
            FluidButton{text:"Cc";onClicked:cc.visible=!cc.visible}
            FluidButton{text:"Bcc";onClicked:bcc.visible=!bcc.visible}
        }
        TextField{id:cc;width:parent.width;height:visible?Theme.unit(37):0;visible:false;placeholderText:"Cc";color:Theme.text;background:Item{} onTextEdited:root.sync()}
        TextField{id:bcc;width:parent.width;height:visible?Theme.unit(37):0;visible:false;placeholderText:"Bcc";color:Theme.text;background:Item{} onTextEdited:root.sync()}
        Rectangle{height:1;width:parent.width;color:Theme.outline}
        TextField{id:subject;width:parent.width;height:Theme.unit(45);placeholderText:"Subject";placeholderTextColor:Theme.faint;color:Theme.text;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold;selectByMouse:true;leftPadding:0;background:Item{} KeyNavigation.tab:body;KeyNavigation.backtab:to;onTextEdited:root.sync()}
        Rectangle{height:1;width:parent.width;color:Theme.outline}
        Row{id:toolbar;height:Theme.unit(44);spacing:Theme.unit(2)
            FluidButton{glyph:"bold";tip:"Bold · Ctrl+B";onClicked:body.font.bold=!body.font.bold}
            FluidButton{glyph:"italic";tip:"Italic · Ctrl+I";onClicked:body.font.italic=!body.font.italic}
            FluidButton{glyph:"underline";tip:"Underline · Ctrl+U";onClicked:body.font.underline=!body.font.underline}
            FluidButton{glyph:"list";tip:"Bulleted list"}
            FluidButton{glyph:"link";tip:"Insert hyperlink"}
            FluidButton{glyph:"attachment";tip:"Attach files"}
        }
    }
    Flickable {
        id:bodyScroll;x:Theme.unit(22);y:fields.y+fields.height;width:parent.width-2*x;height:Math.max(0,footer.y-y-Theme.unit(10));clip:true;contentWidth:width;contentHeight:body.implicitHeight;visible:root.phase==="editing"
        TextArea{id:body;width:bodyScroll.width;implicitHeight:Math.max(bodyScroll.height,contentHeight+Theme.unit(16));padding:0;color:Theme.text;placeholderText:"A little space for your thoughts…";placeholderTextColor:Theme.faint;font.pixelSize:Theme.unit(13);wrapMode:TextEdit.Wrap;selectByMouse:true;selectionColor:Theme.selected;background:Item{} KeyNavigation.backtab:subject;onTextChanged:root.sync()}
        ScrollBar.vertical:ScrollBar{implicitWidth:5}
    }
    Item {
        id:footer;x:Theme.unit(18);y:parent.height-Theme.unit(62);width:parent.width-2*x;height:Theme.unit(48);visible:root.phase==="editing"
        FluidButton{anchors.verticalCenter:parent.verticalCenter;text:"Send";primary:true;glyph:"send";enabled:!!Mail.composer.session;tip:"Send · Ctrl+Enter";onClicked:{root.sync();Mail.send("composeSend",{session:Mail.composer.session})}}
        FluidButton{anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;glyph:"save";tip:"Save to Gmail drafts";enabled:!!Mail.composer.session;onClicked:{root.sync();Mail.send("composeSave",{session:Mail.composer.session})}}
    }
    }
    function sync(){if(formLoading||!shown||!Mail.composer.session)return;Mail.send("composeUpdate",{session:Mail.composer.session,to:to.text,cc:cc.text,bcc:bcc.text,subject:subject.text,text:body.text,html:false})}
    function open(mode){
        if(shown){Qt.callLater(()=>{if(inlineMode)body.forceActiveFocus();else to.forceActiveFocus()});return}
        phase="editing";expanded=false;inlineMode=mode!=="new";shown=true
        if(Theme.motion&&!inlineMode){
            horizontal.snap(origin.x);vertical.snap(origin.y);breadth.snap(origin.width);depth.snap(origin.height)
            horizontal.resume();vertical.resume();breadth.resume();depth.resume()
        }
        Qt.callLater(()=>{if(mode==="new")to.forceActiveFocus();else body.forceActiveFocus()})
    }
    function close(){shown=false}
    function toggleExpanded(){
        const position=mapToItem(scene,0,0)
        if(inlineMode){
            inlineMode=false;parent=scene;expanded=true
            horizontal.snap(position.x);vertical.snap(position.y)
            horizontal.resume();vertical.resume()
        }else if(expanded&&inlineDock){
            const dock=inlineDock.mapToItem(scene,0,0)
            parent=inlineDock;inlineMode=true;expanded=false
            horizontal.snap(position.x-dock.x);vertical.snap(position.y-dock.y)
            horizontal.resume();vertical.resume()
        }else expanded=!expanded
    }
    function dockBottomRight(){manualPosition=false}
    Connections{target:Mail
        function onComposerChanged(){let c=Mail.composer;formLoading=true;to.text=c.to||"";cc.text=c.cc||"";bcc.text=c.bcc||"";subject.text=c.subject||"";body.text=c.text||"";formLoading=false;if(c.phase)phase=c.phase}
    }
}
