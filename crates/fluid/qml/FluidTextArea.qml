import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
T.TextArea {
    id:control
    Accessible.name:placeholderText||(readOnly?"Message text":"Message body")
    property var richDocument:null
    property var pasteAction:null
    property TextEditMenu editMenu:null
    implicitWidth:Math.max(Theme.unit(140),contentWidth+leftPadding+rightPadding)
    implicitHeight:Math.max(Theme.unit(38),contentHeight+topPadding+bottomPadding)
    padding:Theme.unit(8)
    color:Theme.text;placeholderTextColor:Theme.faint
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    selectionColor:Theme.selected;selectedTextColor:Theme.text
    selectByMouse:true
    background:Item{}
    Text{textFormat:Text.PlainText;x:control.leftPadding;y:control.topPadding;width:control.width-control.leftPadding-control.rightPadding;text:control.placeholderText;color:control.placeholderTextColor;font:control.font;visible:!control.length&&!control.preeditText;wrapMode:control.wrapMode}
    Component{id:menuFactory;TextEditMenu{onClosed:destroy()}}
    function openEditMenu(position){
        if(!editMenu)editMenu=menuFactory.createObject(control,{objectName:control.objectName+"ContextMenu",editor:control,richDocument:richDocument,pasteAction:pasteAction})
        editMenu.popup(control,position)
    }
    TapHandler{acceptedButtons:Qt.RightButton;onTapped:event=>control.openEditMenu(event.position)}
    Shortcut{sequences:["Shift+F10","Menu"];enabled:control.activeFocus;onActivated:control.openEditMenu(Qt.point(control.cursorRectangle.x,control.cursorRectangle.y+control.cursorRectangle.height))}
}
