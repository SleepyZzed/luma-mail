import QtQuick
import QtQuick.Controls.Basic
FluidField {
    id:field
    Accessible.name:placeholderText||"Recipients"
    property string session:""
    property string account:""
    property var entries:[]
    property bool suggestionsOpen:false
    property int chosen:0
    property double request:0
    property string requestedPrefix:""
    property string requestedAccount:""
    signal edited()
    color:Theme.text;placeholderTextColor:Theme.faint;selectByMouse:true;font.pixelSize:Theme.unit(12)
    leftPadding:0;rightPadding:0;topPadding:Theme.unit(4);bottomPadding:Theme.unit(4);background:Item{} maximumLength:8192
    onTextEdited:{edited();suggestionsOpen=false;delay.restart()}
    onActiveFocusChanged:if(!activeFocus){suggestionsOpen=false;delay.stop()}else delay.restart()
    onCursorPositionChanged:if(activeFocus){const p=part();if(!p||p.prefix!==requestedPrefix)suggestionsOpen=false;delay.restart()}
    onAccountChanged:{suggestionsOpen=false;request=0;if(activeFocus)delay.restart()}
    onSessionChanged:{suggestionsOpen=false;request=0;entries=[]}
    Timer{id:delay;interval:160;onTriggered:field.query()}
    // Delimiters inside a quoted display name or angle address never split a recipient.
    function part(){
        let quote=false,escape=false,angle=false,start=0,end=text.length
        const caret=cursorPosition
        for(let i=0;i<text.length;i++){
            const ch=text[i]
            if(escape){escape=false;continue}
            if(ch==="\\"&&quote){escape=true;continue}
            if(ch==='"'){quote=!quote;continue}
            if(!quote&&ch==="<"){angle=true;continue}
            if(!quote&&ch===">"){angle=false;continue}
            if(!quote&&!angle&&(ch===","||ch===";")){
                if(i<caret)start=i+1;else{end=i;break}
            }
        }
        const prefix=text.slice(start,caret).trim()
        if(prefix.length<1||prefix.length>320||/["<>\r\n]/.test(prefix))return null
        return {start:start,end:end,prefix:prefix}
    }
    function query(){
        const p=part();if(!activeFocus||!session||!p||Mail.settings.recent_recipients===false){suggestionsOpen=false;return}
        requestedPrefix=p.prefix;requestedAccount=account
        request=Mail.send("composeRecipients",{session:session,prefix:p.prefix})
    }
    function accept(){
        const p=part();if(!suggestionsOpen||!p||p.prefix!==requestedPrefix||account!==requestedAccount||!entries.length)return false
        const item=entries[Math.max(0,Math.min(entries.length-1,chosen))]
        const leading=text.slice(p.start,cursorPosition).match(/^\s*/)[0]
        const extra=p.end===text.length?", ":""
        text=text.slice(0,p.start)+leading+item+extra+text.slice(p.end)
        cursorPosition=p.start+leading.length+item.length+extra.length
        suggestionsOpen=false;delay.stop();edited();forceActiveFocus();return true
    }
    Keys.onPressed:(event)=>{
        if((event.key===Qt.Key_Tab||event.key===Qt.Key_Return||event.key===Qt.Key_Enter)&&!(event.modifiers&Qt.ShiftModifier)&&suggestionsOpen){event.accepted=accept()}
        else if(event.key===Qt.Key_Down&&suggestionsOpen){chosen=Math.min(entries.length-1,chosen+1);event.accepted=true}
        else if(event.key===Qt.Key_Up&&suggestionsOpen){chosen=Math.max(0,chosen-1);event.accepted=true}
        else if(event.key===Qt.Key_Escape&&suggestionsOpen){suggestionsOpen=false;event.accepted=true}
        else if(event.key===Qt.Key_Backtab||(event.key===Qt.Key_Tab&&(event.modifiers&Qt.ShiftModifier))){suggestionsOpen=false;event.accepted=false}
        else event.accepted=false
    }
    Item{
        id:suggestions;objectName:field.objectName+"Suggestions";x:0;y:field.height+Theme.unit(4);width:Math.min(Theme.unit(430),Math.max(field.width,Theme.unit(200)));height:Math.max(0,opening.value);visible:field.suggestionsOpen||opening.running;clip:true;z:100
        Motion{id:opening;target:field.suggestionsOpen?Math.min(5,field.entries.length)*Theme.unit(34)+Theme.unit(10):0;enabled:Theme.motion}
        Rectangle{anchors.fill:parent;color:Theme.raised;radius:Theme.unit(10);border.width:1;border.color:Theme.outline}
        Column{x:Theme.unit(5);y:x;width:parent.width-2*x
            Repeater{model:field.entries
                Rectangle{required property string modelData;required property int index;width:parent.width;height:Theme.unit(34);radius:Theme.unit(6);color:field.chosen===index?Theme.selected:"transparent"
                    Text{textFormat:Text.PlainText;anchors.fill:parent;anchors.leftMargin:Theme.unit(10);anchors.rightMargin:Theme.unit(10);verticalAlignment:Text.AlignVCenter;text:modelData;elide:Text.ElideRight;color:Theme.text;font.pixelSize:Theme.unit(11)}
                    MouseArea{anchors.fill:parent;hoverEnabled:true;onEntered:field.chosen=index;onClicked:{field.chosen=index;field.accept()}}
                }
            }
        }
    }
    Connections{target:Mail
        function onReceived(event,data){
            if(event==="recentCleared"||(event==="settings"&&data.recent_recipients===false)){field.entries=[];field.suggestionsOpen=false;field.request=0;return}
            if(event!=="recipients"||data.session!==field.session||data.request!==field.request||data.account!==field.requestedAccount||data.account!==field.account||!field.activeFocus)return
            const p=field.part();if(!p||p.prefix!==data.prefix)return
            field.entries=data.items||[];field.chosen=0;field.suggestionsOpen=field.entries.length>0
        }
    }
}
